<?xml version="1.0" encoding="UTF-8"?>
<!--UNCLASSIFIED--><xsl:stylesheet xmlns:xs="http://www.w3.org/2001/XMLSchema"
                xmlns:xsd="http://www.w3.org/2001/XMLSchema"
                xmlns:saxon="http://saxon.sf.net/"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:schold="http://www.ascc.net/xml/schematron"
                xmlns:iso="http://purl.oclc.org/dsdl/schematron"
                xmlns:xhtml="http://www.w3.org/1999/xhtml"
                xmlns:ism="urn:us:gov:ic:ism"
                xmlns:ntk="urn:us:gov:ic:ntk"
                xmlns:arh="urn:us:gov:ic:arh"
                xmlns:catt="urn:us:gov:ic:taxonomy:catt:tetragraph"
                xmlns:cve="urn:us:gov:ic:cve"
                xmlns:dvf="deprecated:value:function"
                xmlns:util="urn:us:gov:ic:ism:xsl:util"
                version="2.0"><!--Implementers: please note that overriding process-prolog or process-root is 
    the preferred method for meta-stylesheets to use where possible. -->
<xsl:param name="archiveDirParameter"/>
   <xsl:param name="archiveNameParameter"/>
   <xsl:param name="fileNameParameter"/>
   <xsl:param name="fileDirParameter"/>
   <xsl:variable name="document-uri">
      <xsl:value-of select="document-uri(/)"/>
   </xsl:variable>

   <!--PHASES-->


<!--PROLOG-->
<xsl:output xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
               method="xml"
               omit-xml-declaration="no"
               standalone="yes"
               indent="yes"/>

   <!--XSD TYPES FOR XSLT2-->


<!--KEYS AND FUNCTIONS-->
<xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:contributesToRollup"
                 as="xs:boolean">
      <xsl:param name="context"/>
      <xsl:sequence select="not(string($context/@ism:excludeFromRollup) = string(true()))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:getDissemControlsList"
                 as="node()*">
      <xsl:choose>
         <xsl:when test="($ISM_USGOV_RESOURCE or $ISM_OTHER_AUTH_RESOURCE) and not($ISM_USCUI_RESOURCE)">
            <xsl:copy-of select="document('../../CVE/ISM/CVEnumISMDissemIcrm.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
         </xsl:when>
         <xsl:when test="$ISM_USGOV_RESOURCE and $ISM_USCUI_RESOURCE">
            <xsl:copy-of select="document('../../CVE/ISM/CVEnumISMDissemCommingled.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
         </xsl:when>
         <xsl:when test="$ISM_USCUIONLY_RESOURCE">
            <xsl:copy-of select="document('../../CVE/ISM/CVEnumISMDissemCui.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
         </xsl:when>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="dvf:deprecated"
                 as="xs:string*">
      <xsl:param name="attribute" as="xs:string"/>
      <xsl:param name="depTerms" as="element()*"/>
      <xsl:param name="curDate" as="xs:date?"/>
      <xsl:param name="isError" as="xs:boolean"/>
      
      <xsl:if test="count($curDate) = 1">
         <xsl:for-each select="$depTerms[cve:Value = tokenize($attribute, ' ')]">
            <xsl:if test="($isError and $curDate gt xs:date(@deprecated)) or (not($isError) and $curDate le xs:date(@deprecated))">
               <xsl:sequence select="concat('[', string(current()/cve:Value), '] has been deprecated and is not authorized for use after  ', current()/@deprecated)"/>
            </xsl:if>
         </xsl:for-each>
      </xsl:if>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:containsAnyTokenMatching"
                 as="xs:boolean">
      <xsl:param name="attribute"/>
      <xsl:param name="regexList" as="xs:string+"/>
      <xsl:sequence select="             some $attrToken in tokenize(normalize-space(string($attribute)), ' ')                satisfies (some $regex in $regexList                   satisfies matches($attrToken, $regex))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:containsAnyOfTheTokens"
                 as="xs:boolean">
      <xsl:param name="attribute"/>
      <xsl:param name="tokenList" as="xs:string*"/>
      <xsl:sequence select="             some $attrToken in tokenize(normalize-space(string($attribute)), ' ')                satisfies $attrToken = $tokenList"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:containsOnlyTheTokens"
                 as="xs:boolean">
      <xsl:param name="attribute"/>
      <xsl:param name="tokenList" as="xs:string*"/>
      <xsl:sequence select="             every $attrToken in tokenize(normalize-space(string($attribute)), ' ')                satisfies $attrToken = $tokenList"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:existInTokenSet"
                 as="xs:boolean">
      <xsl:param name="stringTokenValue"/>
      <xsl:param name="tokenList" as="xs:string*"/>
      <xsl:sequence select="tokenize($stringTokenValue, ' ') = $tokenList"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:getStringFromSequenceWithOnlyRegexValues"
                 as="xs:string">
      <xsl:param name="attrValues"/>
      <xsl:param name="regex"/>
      <xsl:variable name="StringWithOnlyRegexValues">
         <xsl:for-each select="$attrValues">
            
            <xsl:if test="matches(current(), $regex)">
               <xsl:value-of select="current()"/>
            </xsl:if>
            <xsl:value-of select="' '"/>
         </xsl:for-each>
      </xsl:variable>
      <xsl:sequence select="normalize-space(string($StringWithOnlyRegexValues))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:getStringFromSequenceWithoutRegexValues"
                 as="xs:string">
      <xsl:param name="attrValues"/>
      <xsl:param name="regex"/>
      <xsl:variable name="StringWithoutRegexValues">
         <xsl:for-each select="$attrValues">
            
            <xsl:if test="not(matches(current(), $regex))">
               <xsl:value-of select="current()"/>
            </xsl:if>
            <xsl:value-of select="' '"/>
         </xsl:for-each>
      </xsl:variable>
      <xsl:sequence select="normalize-space(string($StringWithoutRegexValues))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:getStringFromSequence"
                 as="xs:string">
      <xsl:param name="attrValues"/>
      <xsl:variable name="StringValues">
         <xsl:for-each select="$attrValues">
            <xsl:value-of select="current()"/>
            <xsl:value-of select="' '"/>
         </xsl:for-each>
      </xsl:variable>
      <xsl:sequence select="normalize-space(string($StringValues))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:nonalphabeticValues"
                 as="xs:string">
      <xsl:param name="attrValues"/>
      <xsl:variable name="badValues">
         <xsl:for-each select="$attrValues">
            
            <xsl:if test="not(index-of($attrValues, current())[last()] = count($attrValues))">
               
               <xsl:if test="compare(current(), $attrValues[index-of($attrValues, current()) + 1]) = 1">
                  <xsl:value-of select="$attrValues[index-of($attrValues, current()) + 1]"/>
               </xsl:if>
               <xsl:value-of select="' '"/>
            </xsl:if>
         </xsl:for-each>
      </xsl:variable>
      <xsl:sequence select="normalize-space(string($badValues))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:relativeOrderBetweenACCMAndNonACCMWhenExcludeFromRollup"
                 as="xs:string">
      <xsl:param name="attrValues" as="xs:string*"/>

      <xsl:variable name="badValues">
         <xsl:for-each select="$attrValues">
            
            <xsl:if test="not(index-of($attrValues, current())[last()] = count($attrValues))">
               
               <xsl:if test="not(matches(current(), $ACCMRegex)) and matches($attrValues[index-of($attrValues, current()) + 1], $ACCMRegex) and not(util:existInTokenSet(current(), $nonACCMLeftSetTok))">
                  <xsl:value-of select="current()"/>
               </xsl:if>
               
               <xsl:if test="matches(current(), $ACCMRegex) and not(matches($attrValues[index-of($attrValues, current()) + 1], $ACCMRegex)) and not(util:existInTokenSet($attrValues[index-of($attrValues, current()) + 1], $nonACCMRightSetTok))">
                  <xsl:value-of select="$attrValues[index-of($attrValues, current()) + 1]"/>
               </xsl:if>
               <xsl:value-of select="' '"/>
            </xsl:if>
         </xsl:for-each>
      </xsl:variable>
      <xsl:sequence select="normalize-space(string($badValues))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:unorderedValues"
                 as="xs:string">
      <xsl:param name="attrValues" as="xs:string*"/>
      <xsl:param name="tokenList" as="xs:string*"/>

      <xsl:variable name="badValues">
         <xsl:for-each select="$attrValues">
            
            <xsl:if test="not(index-of($attrValues, current())[last()] = count($attrValues))">

               
               <xsl:variable name="indexOfValue"
                             select="util:getIndexFromListMatch(current(), $tokenList)"/>
               <xsl:variable name="indexOfNextValue"
                             select="util:getIndexFromListMatch($attrValues[index-of($attrValues, current()) + 1], $tokenList)"/>


               <xsl:choose>
                  <xsl:when test="$indexOfValue = $indexOfNextValue">
                     
                     
                     <xsl:if test="compare(current(), $attrValues[index-of($attrValues, current()) + 1]) = 1">
                        <xsl:value-of select="$attrValues[index-of($attrValues, current()) + 1]"/>
                     </xsl:if>
                  </xsl:when>
                  <xsl:when test="$indexOfValue &gt; $indexOfNextValue">
                     
                     <xsl:value-of select="$attrValues[index-of($attrValues, current()) + 1]"/>
                  </xsl:when>
               </xsl:choose>
               <xsl:value-of select="' '"/>
            </xsl:if>
         </xsl:for-each>
      </xsl:variable>
      <xsl:sequence select="normalize-space(string($badValues))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:unsortedValues"
                 as="xs:string">
      <xsl:param name="attribute"/>
      <xsl:param name="tokenList" as="xs:string*"/>
      <xsl:variable name="attrValues"
                    select="tokenize(normalize-space(string($attribute)), ' ')"/>

      <xsl:variable name="badValues">
         <xsl:for-each select="$attrValues">
            
            <xsl:if test="not(index-of($attrValues, current())[last()] = count($attrValues))">

               
               <xsl:variable name="indexOfValue"
                             select="util:getIndexFromListMatch(current(), $tokenList)"/>
               <xsl:variable name="indexOfNextValue"
                             select="util:getIndexFromListMatch($attrValues[index-of($attrValues, current()) + 1], $tokenList)"/>


               <xsl:choose>
                  <xsl:when test="$indexOfValue = $indexOfNextValue">
                     
                     
                     <xsl:if test="compare(current(), $attrValues[index-of($attrValues, current()) + 1]) = 1">
                        <xsl:value-of select="$attrValues[index-of($attrValues, current()) + 1]"/>
                     </xsl:if>
                  </xsl:when>
                  <xsl:when test="$indexOfValue &gt; $indexOfNextValue">
                     
                     <xsl:value-of select="$attrValues[index-of($attrValues, current()) + 1]"/>
                  </xsl:when>
               </xsl:choose>
               <xsl:value-of select="' '"/>
            </xsl:if>
         </xsl:for-each>
      </xsl:variable>
      <xsl:sequence select="normalize-space(string($badValues))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:getIndexFromListMatch"
                 as="xs:integer">
      <xsl:param name="value" as="xs:string"/>
      <xsl:param name="list" as="xs:string*"/>

      <xsl:variable name="index">
         <xsl:for-each select="$list">
            <xsl:if test="matches($value, concat('^', current(), '$'))">
               <xsl:value-of select="index-of($list, current())[1]"/>
            </xsl:if>
         </xsl:for-each>
      </xsl:variable>

      <xsl:choose>
         <xsl:when test="$index = ''">
            <xsl:sequence select="xs:integer(-1)"/>
         </xsl:when>
         <xsl:otherwise>
            <xsl:sequence select="xs:integer(number($index[1]))"/>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:meetsType"
                 as="xs:boolean">
      <xsl:param name="value"/>
      <xsl:param name="typePattern" as="xs:string"/>
      <xsl:sequence select="matches(normalize-space(string($value)), concat('^(', $typePattern, ')$'))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:getCountriesForTetra"
                 as="xs:string*">
      <xsl:param name="tetra" as="xs:string"/>

      <xsl:sequence select="$decomposableTetraElems[catt:TetraToken/text() = $tetra]/catt:Membership/*/text()"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:padValue"
                 as="xs:string">
      <xsl:param name="value" as="xs:string?"/>

      <xsl:sequence select="concat(' ', normalize-space($value), ' ')"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:tokenize"
                 as="xs:string*">
      <xsl:param name="value" as="xs:string?"/>

      <xsl:sequence select="tokenize(normalize-space($value), ' ')"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:join"
                 as="xs:string">
      <xsl:param name="values" as="xs:string*"/>

      <xsl:sequence select="normalize-space(string-join($values, ' '))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:sort"
                 as="xs:string*">
      <xsl:param name="values" as="xs:string*"/>

      <xsl:variable name="sortedValues">
         <xsl:for-each select="$values">
            <xsl:sort select="."/>
            <xsl:value-of select="util:padValue(.)"/>
         </xsl:for-each>
      </xsl:variable>

      <xsl:sequence select="util:tokenize($sortedValues)"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:countIn"
                 as="xs:double">
      <xsl:param name="value" as="xs:string"/>
      <xsl:param name="expandedRelToStrings" as="xs:string*"/>
      <xsl:param name="countryHash" as="item()*"/>

      <xsl:variable name="counts" as="xs:integer*">
         <xsl:for-each select="$expandedRelToStrings">
            <xsl:if test="util:containsAnyOfTheTokens(., $value)">
               
               <xsl:variable name="expandedPosition" select="position()"/>
               <xsl:sequence select="$countryHash[position() = $expandedPosition * 2]"/>
            </xsl:if>
         </xsl:for-each>
      </xsl:variable>

      <xsl:sequence select="sum($counts)"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:isSubsetOf"
                 as="xs:boolean">
      <xsl:param name="subset" as="xs:string*"/>
      <xsl:param name="superset" as="xs:string*"/>

      <xsl:sequence select="             (every $subsetToken in $subset                satisfies $subsetToken = $superset)"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:containsDecomposableTetra"
                 as="xs:boolean">
      <xsl:param name="relTo" as="xs:string?"/>

      <xsl:sequence select="normalize-space($relTo) and util:containsAnyOfTheTokens($relTo, $decomposableTetras)"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:expandAllTetras"
                 as="xs:string*">
      <xsl:param name="relToStrings" as="xs:string*"/>

      <xsl:variable name="allTokens" as="xs:string*">
         <xsl:for-each select="$relToStrings">
            <xsl:variable name="expandedCountryTokens" select="util:expandDecomposableTetras(.)"/>
            <xsl:value-of select="util:padValue(util:join($expandedCountryTokens))"/>
         </xsl:for-each>
      </xsl:variable>

      <xsl:sequence select="$allTokens"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:expandDecomposableTetras"
                 as="xs:string*">
      <xsl:param name="relTo" as="xs:string"/>

      <xsl:variable name="expandedTetras">
         <xsl:choose>
            <xsl:when test="util:containsDecomposableTetra($relTo)">
               <xsl:variable name="currTetra"
                             select="util:tokenize($relTo)[. = $decomposableTetras][1]"/>
               <xsl:variable name="currTetraCountries"
                             select="util:join(util:getCountriesForTetra($currTetra))"/>
               <xsl:variable name="expandCurrTetra"
                             select="replace(util:padValue($relTo), util:padValue($currTetra), util:padValue($currTetraCountries))"/>

               <xsl:value-of select="util:expandDecomposableTetras($expandCurrTetra)"/>
            </xsl:when>

            <xsl:otherwise>
               <xsl:value-of select="normalize-space($relTo)"/>
            </xsl:otherwise>
         </xsl:choose>
      </xsl:variable>

      <xsl:sequence select="distinct-values(util:tokenize($expandedTetras))[. != 'USA']"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:createCountryHash"
                 as="item()*">
      <xsl:param name="relToStrings" as="xs:string*"/>

      <xsl:for-each-group select="$relToStrings" group-by=".">
         <xsl:sequence select="current-grouping-key(), count(current-group())"/>
      </xsl:for-each-group>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:calculateCommonCountries"
                 as="xs:string*">
      <xsl:param name="portionCountryStrings" as="xs:string*"/>

      
      <xsl:variable name="countryHash"
                    select="util:createCountryHash($portionCountryStrings)"/>

      
      <xsl:variable name="expandedTetras"
                    select="util:expandAllTetras($countryHash[position() mod 2 = 1])"/>
      <xsl:variable name="distinctCountryTokens"
                    select="distinct-values(util:tokenize(util:join($expandedTetras)))[. != 'USA']"/>

      
      <xsl:sequence select="$distinctCountryTokens[util:countIn(., $expandedTetras, $countryHash) = $countFdrPortions]"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:decomposeTetragraphs"
                 as="xs:string*">
      <xsl:param name="releasableTo" as="xs:string"/>
      <xsl:sequence select="             for $token in tokenize(normalize-space($releasableTo), ' ')             return                if (util:isTetragraph($token)) then                   util:getTetragraphMembership($token)                else                   $token"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:isTetragraph"
                 as="xs:boolean">
      <xsl:param name="value" as="xs:string"/>

      <xsl:sequence select="             some $token in $tetragraphList                satisfies $token = $value"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:before-last-delimeter">
      <xsl:param name="s"/>
      <xsl:param name="d"/>

      <xsl:variable name="s-tokenized" select="tokenize($s, $d)"/>
      <xsl:sequence select="string-join(remove($s-tokenized, count($s-tokenized)), $d)"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:containsSpecialTetra"
                 as="xs:boolean">
      <xsl:param name="releasableTo" as="xs:string"/>
      
      <xsl:sequence select="             some $token in tokenize(normalize-space($releasableTo), ' ')                satisfies util:isTetragraph($token) and $catt//catt:Tetragraph[catt:TetraToken = $token]/@decomposable[not(. = 'Yes' or . = 'Maybe' or . = 'NA')]"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:containsMaybeTetra"
                 as="xs:boolean">
      <xsl:param name="releasableTo" as="xs:string"/>
      <xsl:sequence select="             some $token in tokenize(normalize-space($releasableTo), ' ')                satisfies util:isTetragraph($token) and $catt//catt:Tetragraph[catt:TetraToken = $token]/@decomposable[. = 'Maybe']"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:relToContainsMaybeTetra"
                 as="xs:boolean">
      <xsl:param name="bannerRelTo" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count($remainingPartTags) = 0">
            
            <xsl:sequence select="xs:boolean(false())"/>
         </xsl:when>
         <xsl:when test="$bannerRelTo and util:containsMaybeTetra($bannerRelTo)">
            <xsl:sequence select="xs:boolean(true())"/>
         </xsl:when>
         <xsl:when test="$portion/@ism:releasableTo and util:containsMaybeTetra($portion/@ism:releasableTo)">
            <xsl:sequence select="xs:boolean(true())"/>
         </xsl:when>
         <xsl:otherwise>
            <xsl:sequence select="xs:boolean(util:relToContainsMaybeTetraHelper($bannerRelTo, subsequence($remainingPartTags, 2)))"/>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:relToContainsMaybeTetraHelper"
                 as="xs:string*">
      <xsl:param name="bannerRelTo" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count($remainingPartTags) = 1">
            
            <xsl:sequence select="xs:string(util:relToContainsMaybeTetra($bannerRelTo, ()))"/>
         </xsl:when>
         <xsl:otherwise>
            
            <xsl:sequence select="xs:string(util:relToContainsMaybeTetra($bannerRelTo, subsequence($remainingPartTags, 2)))"/>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:displayToContainsMaybeTetra"
                 as="xs:boolean">
      <xsl:param name="bannerDisplayTo" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count($remainingPartTags) = 0">
            
            <xsl:sequence select="xs:boolean(false())"/>
         </xsl:when>
         <xsl:when test="$bannerDisplayTo and util:containsMaybeTetra($bannerDisplayTo)">
            <xsl:sequence select="xs:boolean(true())"/>
         </xsl:when>
         <xsl:when test="$portion/@ism:displayOnlyTo and util:containsMaybeTetra($portion/@ism:displayOnlyTo)">
            <xsl:sequence select="xs:boolean(true())"/>
         </xsl:when>
         <xsl:otherwise>
            <xsl:sequence select="xs:boolean(util:displayToContainsMaybeTetra($bannerDisplayTo, subsequence($remainingPartTags, 2)))"/>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:displayToContainsMaybeTetraHelper"
                 as="xs:string*">
      <xsl:param name="bannerDisplayTo" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count($remainingPartTags) = 1">
            
            <xsl:sequence select="xs:string(util:displayToContainsMaybeTetra($bannerDisplayTo, ()))"/>
         </xsl:when>
         <xsl:otherwise>
            
            <xsl:sequence select="xs:string(util:displayToContainsMaybeTetra($bannerDisplayTo, subsequence($remainingPartTags, 2)))"/>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:bannerIsSubset"
                 as="xs:boolean">
      <xsl:param name="bannerRelTo" as="xs:string"/>
      <xsl:param name="portionRelTo" as="xs:string"/>
      <xsl:variable name="bannerRelToDecomposed"
                    select="tokenize(normalize-space(util:decomposeTetragraphs($bannerRelTo)), ' ')"/>
      <xsl:variable name="portionRelToDecomposed"
                    select="tokenize(normalize-space(util:decomposeTetragraphs($portionRelTo)), ' ')"/>
      <xsl:sequence select="             util:containsSpecialTetra($bannerRelTo) or (every $bannerToken in $bannerRelToDecomposed                satisfies (some $portionToken in $portionRelToDecomposed                   satisfies if ($bannerToken = 'USA') then                      true()                   else                      $bannerToken = $portionToken))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:containsFDR"
                 as="xs:boolean">
      <xsl:param name="elementNode" as="node()"/>
      <xsl:sequence select="$elementNode/@ism:releasableTo or $elementNode/@ism:displayOnlyTo or util:containsAnyOfTheTokens($elementNode/@ism:disseminationControls, ('NF', 'RELIDO')) or util:containsAnyOfTheTokens($elementNode/@ism:nonICmarkings, ('LES-NF', 'SBU-NF'))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:intersectionOfCountries"
                 as="xs:string*">
      <xsl:param name="commonCountries" as="xs:string"/>
      <xsl:param name="portionRelTo" as="xs:string"/>
      <xsl:variable name="portionRelToDecomposed"
                    select="tokenize(normalize-space(util:decomposeTetragraphs($portionRelTo)), ' ')"/>
      <xsl:sequence select="             for $token in tokenize(normalize-space($commonCountries), ' ')             return                if ($token = $portionRelToDecomposed and not($token = 'USA')) then                   $token                else                   ()"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:recursivelyCheckRelTo"
                 as="xs:string*">
      <xsl:param name="bannerRelTo" as="xs:string"/>
      <xsl:param name="commonCountries" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count(tokenize($commonCountries, ' ')) = 0">
            
            <xsl:sequence select="()"/>
         </xsl:when>
         <xsl:when test="count($remainingPartTags) = 0">
            
            <xsl:sequence select="$commonCountries"/>
         </xsl:when>
         <xsl:when test="not(util:containsFDR($portion)) and $portion/@ism:classification = 'U'">
            
            <xsl:sequence select="util:recursivelyCheckRelTo($bannerRelTo, $commonCountries, subsequence($remainingPartTags, 2))"/>
         </xsl:when>
         <xsl:when test="not($portion/@ism:releasableTo)">
            
            <xsl:sequence select="()"/>
         </xsl:when>
         <xsl:when test="util:containsSpecialTetra($portion/@ism:releasableTo)">
            
            <xsl:sequence select="util:recursivelyCheckRelTo($bannerRelTo, $commonCountries, subsequence($remainingPartTags, 2))"/>
         </xsl:when>
         <xsl:otherwise>
            
            <xsl:choose>
               <xsl:when test="util:bannerIsSubset($bannerRelTo, $portion/@ism:releasableTo)">
                  
                  <xsl:sequence select="util:recursivelyCheckRelToRecurseHelper($bannerRelTo, $commonCountries, $remainingPartTags)"/>
               </xsl:when>
               <xsl:otherwise>
                  
                  <xsl:sequence select="('BANNER_NOT_A_SUBSET_OF_A_PORTION')"/>
               </xsl:otherwise>
            </xsl:choose>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:recursivelyCheckRelToRecurseHelper"
                 as="xs:string*">
      <xsl:param name="bannerRelTo" as="xs:string"/>
      <xsl:param name="commonCountries" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count($remainingPartTags) = 1">
            
            <xsl:sequence select="util:recursivelyCheckRelTo($bannerRelTo, util:intersectionOfCountries($commonCountries, $portion/@ism:releasableTo), ())"/>
         </xsl:when>
         <xsl:otherwise>
            
            <xsl:sequence select="util:recursivelyCheckRelTo($bannerRelTo, util:intersectionOfCountries($commonCountries, $portion/@ism:releasableTo), subsequence($remainingPartTags, 2))"/>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:isUncaveatedAndNoFDR"
                 as="xs:boolean">
      <xsl:param name="element"/>
      <xsl:sequence select="not($element/@ism:disseminationControls) and not($element/@ism:SCIcontrols) and not($element/@ism:nonICmarkings) and not($element/@ism:atomicEnergyMarkings) and not($element/@ism:FGIsourceOpen) and not($element/@ism:FGIsourceProtected) and not($element/@ism:nonUSControls) and not($element/@ism:SARIdentifier)"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:checkRelToPortionsAgainstBannerAndGetCommonCountries"
                 as="xs:string*">
      <xsl:param name="bannerRelTo" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count($remainingPartTags) = 0">
            
            <xsl:sequence select="('PASS')"/>
         </xsl:when>
         <xsl:when test="util:containsFDR($portion) and not($portion/@ism:releasableTo)">
            

            <xsl:sequence select="()"/>
         </xsl:when>
         <xsl:when test="$portion/@ism:releasableTo and not(util:containsSpecialTetra($portion/@ism:releasableTo))">
            
            <xsl:sequence select="util:recursivelyCheckRelTo($bannerRelTo, util:decomposeTetragraphs($portion/@ism:releasableTo), $remainingPartTags)"/>

         </xsl:when>
         <xsl:otherwise>
            
            <xsl:sequence select="util:checkRelToPortionsAgainstBannerAndGetCommonCountries($bannerRelTo, subsequence($remainingPartTags, 2))"/>

         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:getDisplayToCountries">
      <xsl:param name="portion" as="node()"/>
      <xsl:sequence select="normalize-space(concat(normalize-space(string($portion/@ism:releasableTo)), ' ', normalize-space(string($portion/@ism:displayOnlyTo))))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:isDisplayable"
                 as="xs:boolean">
      <xsl:param name="portion" as="node()"/>
      <xsl:sequence select="$portion/@ism:releasableTo or $portion/@ism:displayOnlyTo"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:recursivelyCheckDisplayTo"
                 as="xs:string*">
      <xsl:param name="bannerRelToAndDisplayTo" as="xs:string"/>
      <xsl:param name="commonCountries" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count(tokenize($commonCountries, ' ')) = 0">
            
            <xsl:sequence select="()"/>
         </xsl:when>
         <xsl:when test="count($remainingPartTags) = 0">
            
            <xsl:sequence select="$commonCountries"/>
         </xsl:when>
         <xsl:when test="not(util:containsFDR($portion)) and $portion/@ism:classification = 'U'">
            
            <xsl:sequence select="util:recursivelyCheckDisplayTo($bannerRelToAndDisplayTo, $commonCountries, subsequence($remainingPartTags, 2))"/>
         </xsl:when>
         <xsl:when test="not($portion/@ism:releasableTo) and not($portion/@ism:displayOnlyTo)">
            
            <xsl:sequence select="()"/>
         </xsl:when>
         <xsl:when test="util:containsSpecialTetra(util:getDisplayToCountries($portion))">
            
            <xsl:sequence select="util:recursivelyCheckDisplayTo($bannerRelToAndDisplayTo, $commonCountries, subsequence($remainingPartTags, 2))"/>
         </xsl:when>
         <xsl:otherwise>
            
            <xsl:choose>
               <xsl:when test="util:bannerIsSubset($bannerRelToAndDisplayTo, util:getDisplayToCountries($portion))">
                  
                  <xsl:sequence select="util:recursivelyCheckDisplayToRecurseHelper($bannerRelToAndDisplayTo, $commonCountries, $remainingPartTags)"/>
               </xsl:when>
               <xsl:otherwise>
                  
                  <xsl:sequence select="('BANNER_NOT_A_SUBSET_OF_A_PORTION')"/>
               </xsl:otherwise>
            </xsl:choose>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:recursivelyCheckDisplayToRecurseHelper"
                 as="xs:string*">
      <xsl:param name="bannerRelToAndDisplayTo" as="xs:string"/>
      <xsl:param name="commonCountries" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count($remainingPartTags) = 1">
            
            <xsl:sequence select="util:recursivelyCheckDisplayTo($bannerRelToAndDisplayTo, util:intersectionOfCountries($commonCountries, util:getDisplayToCountries($portion)), ())"/>
         </xsl:when>
         <xsl:otherwise>
            
            <xsl:sequence select="util:recursivelyCheckDisplayTo($bannerRelToAndDisplayTo, util:intersectionOfCountries($commonCountries, util:getDisplayToCountries($portion)), subsequence($remainingPartTags, 2))"/>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:checkDisplayToPortionsAgainstBannerAndGetCommonCountries"
                 as="xs:string*">
      <xsl:param name="bannerRelToAndDisplayTo" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count($remainingPartTags) = 0">
            
            <xsl:sequence select="('PASS')"/>
         </xsl:when>
         <xsl:when test="util:containsFDR($portion) and not(util:isDisplayable($portion))">
            
            <xsl:sequence select="()"/>
         </xsl:when>
         <xsl:when test="util:isDisplayable($portion) and not(util:containsSpecialTetra(util:getDisplayToCountries($portion)))">
            
            <xsl:sequence select="util:recursivelyCheckDisplayTo($bannerRelToAndDisplayTo, util:decomposeTetragraphs(util:getDisplayToCountries($portion)), $remainingPartTags)"/>
         </xsl:when>
         <xsl:otherwise>
            
            <xsl:sequence select="util:checkDisplayToPortionsAgainstBannerAndGetCommonCountries($bannerRelToAndDisplayTo, subsequence($remainingPartTags, 2))"/>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:getTetragraphMembership">
      <xsl:param name="tetra"/>
      <xsl:variable name="tetragraph"
                    select="$catt//catt:Tetragraph[catt:TetraToken = $tetra]"/>
      <xsl:value-of select="             if ($tetragraph[@decomposable = 'Yes' or @decomposable = 'NA'])             then                string-join(($tetragraph/catt:Membership/*/text()), ' ')             else                $tetra"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:getTetragraphReleasability">
      <xsl:param name="tetra"/>
      <xsl:value-of select="             string-join(distinct-values(for $token in tokenize($catt//catt:Tetragraph[catt:TetraToken = $tetra]/@ism:releasableTo, ' ')             return                if (index-of($catt//catt:TetraToken, $token) &gt; 0) then                   util:getTetragraphMembership($token)                else                   $token), ' ')"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:countSARmarkings">
      <xsl:param name="sars"/>

      <xsl:variable name="tokenizedSARs" select="tokenize($sars,' ')"/>

      <xsl:variable name="SARmarkings">

         <xsl:for-each select="$tokenizedSARs">

            <xsl:if test="not(position() = 1)">
               <xsl:text> </xsl:text>
            </xsl:if>

            <xsl:variable name="SARlessOwner" select="substring-after(.,':')"/>

            <xsl:choose>
               <xsl:when test="contains($SARlessOwner, ':')">
                  <xsl:value-of select="concat(substring-before(.,':'),':',substring-after($SARlessOwner,':'))"/>
               </xsl:when>
               <xsl:otherwise>
                  <xsl:value-of select="."/>
               </xsl:otherwise>
            </xsl:choose>
         </xsl:for-each>
      </xsl:variable>

      <xsl:value-of select="count(distinct-values(tokenize($SARmarkings,' ')))"/>
   </xsl:function>

   <!--DEFAULT RULES-->


<!--MODE: SCHEMATRON-SELECT-FULL-PATH-->
<!--This mode can be used to generate an ugly though full XPath for locators-->
<xsl:template match="*" mode="schematron-select-full-path">
      <xsl:apply-templates select="." mode="schematron-get-full-path"/>
   </xsl:template>

   <!--MODE: SCHEMATRON-FULL-PATH-->
<!--This mode can be used to generate an ugly though full XPath for locators-->
<xsl:template match="*" mode="schematron-get-full-path">
      <xsl:apply-templates select="parent::*" mode="schematron-get-full-path"/>
      <xsl:text>/</xsl:text>
      <xsl:choose>
         <xsl:when test="namespace-uri()=''">
            <xsl:value-of select="name()"/>
         </xsl:when>
         <xsl:otherwise>
            <xsl:text>*:</xsl:text>
            <xsl:value-of select="local-name()"/>
            <xsl:text>[namespace-uri()='</xsl:text>
            <xsl:value-of select="namespace-uri()"/>
            <xsl:text>']</xsl:text>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:variable name="preceding"
                    select="count(preceding-sibling::*[local-name()=local-name(current())                                   and namespace-uri() = namespace-uri(current())])"/>
      <xsl:text>[</xsl:text>
      <xsl:value-of select="1+ $preceding"/>
      <xsl:text>]</xsl:text>
   </xsl:template>
   <xsl:template match="@*" mode="schematron-get-full-path">
      <xsl:apply-templates select="parent::*" mode="schematron-get-full-path"/>
      <xsl:text>/</xsl:text>
      <xsl:choose>
         <xsl:when test="namespace-uri()=''">@<xsl:value-of select="name()"/>
         </xsl:when>
         <xsl:otherwise>
            <xsl:text>@*[local-name()='</xsl:text>
            <xsl:value-of select="local-name()"/>
            <xsl:text>' and namespace-uri()='</xsl:text>
            <xsl:value-of select="namespace-uri()"/>
            <xsl:text>']</xsl:text>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:template>

   <!--MODE: SCHEMATRON-FULL-PATH-2-->
<!--This mode can be used to generate prefixed XPath for humans-->
<xsl:template match="node() | @*" mode="schematron-get-full-path-2">
      <xsl:for-each select="ancestor-or-self::*">
         <xsl:text>/</xsl:text>
         <xsl:value-of select="name(.)"/>
         <xsl:if test="preceding-sibling::*[name(.)=name(current())]">
            <xsl:text>[</xsl:text>
            <xsl:value-of select="count(preceding-sibling::*[name(.)=name(current())])+1"/>
            <xsl:text>]</xsl:text>
         </xsl:if>
      </xsl:for-each>
      <xsl:if test="not(self::*)">
         <xsl:text/>/@<xsl:value-of select="name(.)"/>
      </xsl:if>
   </xsl:template>
   <!--MODE: SCHEMATRON-FULL-PATH-3-->
<!--This mode can be used to generate prefixed XPath for humans 
	(Top-level element has index)-->
<xsl:template match="node() | @*" mode="schematron-get-full-path-3">
      <xsl:for-each select="ancestor-or-self::*">
         <xsl:text>/</xsl:text>
         <xsl:value-of select="name(.)"/>
         <xsl:if test="parent::*">
            <xsl:text>[</xsl:text>
            <xsl:value-of select="count(preceding-sibling::*[name(.)=name(current())])+1"/>
            <xsl:text>]</xsl:text>
         </xsl:if>
      </xsl:for-each>
      <xsl:if test="not(self::*)">
         <xsl:text/>/@<xsl:value-of select="name(.)"/>
      </xsl:if>
   </xsl:template>

   <!--MODE: GENERATE-ID-FROM-PATH -->
<xsl:template match="/" mode="generate-id-from-path"/>
   <xsl:template match="text()" mode="generate-id-from-path">
      <xsl:apply-templates select="parent::*" mode="generate-id-from-path"/>
      <xsl:value-of select="concat('.text-', 1+count(preceding-sibling::text()), '-')"/>
   </xsl:template>
   <xsl:template match="comment()" mode="generate-id-from-path">
      <xsl:apply-templates select="parent::*" mode="generate-id-from-path"/>
      <xsl:value-of select="concat('.comment-', 1+count(preceding-sibling::comment()), '-')"/>
   </xsl:template>
   <xsl:template match="processing-instruction()" mode="generate-id-from-path">
      <xsl:apply-templates select="parent::*" mode="generate-id-from-path"/>
      <xsl:value-of select="concat('.processing-instruction-', 1+count(preceding-sibling::processing-instruction()), '-')"/>
   </xsl:template>
   <xsl:template match="@*" mode="generate-id-from-path">
      <xsl:apply-templates select="parent::*" mode="generate-id-from-path"/>
      <xsl:value-of select="concat('.@', name())"/>
   </xsl:template>
   <xsl:template match="*" mode="generate-id-from-path" priority="-0.5">
      <xsl:apply-templates select="parent::*" mode="generate-id-from-path"/>
      <xsl:text>.</xsl:text>
      <xsl:value-of select="concat('.',name(),'-',1+count(preceding-sibling::*[name()=name(current())]),'-')"/>
   </xsl:template>

   <!--MODE: GENERATE-ID-2 -->
<xsl:template match="/" mode="generate-id-2">U</xsl:template>
   <xsl:template match="*" mode="generate-id-2" priority="2">
      <xsl:text>U</xsl:text>
      <xsl:number level="multiple" count="*"/>
   </xsl:template>
   <xsl:template match="node()" mode="generate-id-2">
      <xsl:text>U.</xsl:text>
      <xsl:number level="multiple" count="*"/>
      <xsl:text>n</xsl:text>
      <xsl:number count="node()"/>
   </xsl:template>
   <xsl:template match="@*" mode="generate-id-2">
      <xsl:text>U.</xsl:text>
      <xsl:number level="multiple" count="*"/>
      <xsl:text>_</xsl:text>
      <xsl:value-of select="string-length(local-name(.))"/>
      <xsl:text>_</xsl:text>
      <xsl:value-of select="translate(name(),':','.')"/>
   </xsl:template>
   <!--Strip characters--><xsl:template match="text()" priority="-1"/>

   <!--SCHEMA SETUP-->
<xsl:template match="/">
      <svrl:schematron-output xmlns:svrl="http://purl.oclc.org/dsdl/svrl" title="" schemaVersion="">
         <xsl:attribute name="phase">TYPECHECK</xsl:attribute>
         <xsl:comment>
            <xsl:value-of select="$archiveDirParameter"/>   
		 <xsl:value-of select="$archiveNameParameter"/>  
		 <xsl:value-of select="$fileNameParameter"/>  
		 <xsl:value-of select="$fileDirParameter"/>
         </xsl:comment>
         <svrl:text> This is the root file for
      the specifications Schematron ruleset. It loads all of the required CVEs, declares some
      variables, and includes all of the Rule .sch files.</svrl:text>
         <svrl:ns-prefix-in-attribute-values uri="urn:us:gov:ic:ism" prefix="ism"/>
         <svrl:ns-prefix-in-attribute-values uri="urn:us:gov:ic:ntk" prefix="ntk"/>
         <svrl:ns-prefix-in-attribute-values uri="urn:us:gov:ic:arh" prefix="arh"/>
         <svrl:ns-prefix-in-attribute-values uri="urn:us:gov:ic:taxonomy:catt:tetragraph" prefix="catt"/>
         <svrl:ns-prefix-in-attribute-values uri="urn:us:gov:ic:cve" prefix="cve"/>
         <svrl:ns-prefix-in-attribute-values uri="deprecated:value:function" prefix="dvf"/>
         <svrl:ns-prefix-in-attribute-values uri="urn:us:gov:ic:ism:xsl:util" prefix="util"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00268</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00268</xsl:attribute>
            <svrl:text>
		[ISM-ID-00268][Error] All @ism:atomicEnergyMarkings attributes must be of type NmTokens. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an atomicEnergyMarkings attribute, this rule ensures that the @ism:atomicEnergyMarkings 
	  	value matches the pattern defined for type NmTokens. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M403"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00269</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00269</xsl:attribute>
            <svrl:text>
		[ISM-ID-00269][Error] All @ism:classification attributes must be of type NmToken. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:classification attribute, this rule ensures that the classification value matches the pattern
		defined for type NmTokens.  
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M404"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00270</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00270</xsl:attribute>
            <svrl:text>
		[ISM-ID-00270][Error] All @ism:classificationReason attributes must be a string with 4096 characters or less. 
	</svrl:text>
            <svrl:text>
		For all elements which contain an @ism:classificationReason attribute, this
		rule ensures that the classificationReason value is a string with 4096 characters or less. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M405"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00271</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00271</xsl:attribute>
            <svrl:text>
		[ISM-ID-00271][Error] All @ism:classifiedBy attributes must be a string with less than 1024 characters. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:classifiedBy attribute, this rule ensures that the classifiedBy value is a string with less
		than 1024 characters.   
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M406"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00272</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00272</xsl:attribute>
            <svrl:text>
		[ISM-ID-00272][Error] All @ism:compilationReason attributes must be a string with less than 1024 characters. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:compilationReason attribute, this rule ensures that the compilationReason value is a string with less
		than 1024 characters.   
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M407"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00273</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00273</xsl:attribute>
            <svrl:text>
		[ISM-ID-00273][Error] All @ism:exemptFrom attributes must be of type NmTokens. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:exemptFrom attribute, this rule ensures that the exemptFrom value matches the pattern
		defined for type NmTokens. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M408"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00274</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00274</xsl:attribute>
            <svrl:text>
		[ISM-ID-00274][Error] All @ism:createDate attributes must be a Date without a timezone.
	</svrl:text>
            <svrl:text>
		For all elements which contain a @ism:createDate attribute, this rule ensures that
		the createDate value matches the pattern defined for type Date without timezone information.
		The value must conform to the Regex ‘[0-9]{4}-[0-9]{2}-[0-9]{2}$’
	</svrl:text>
            <svrl:text>
		The first assert in this rule is not able to be failed in unit tests. If
		the @ism:createDate does not conform to type Date, schematron fails when defining global
		variables before any rules are fired. The first assert is included as a normative statement
		of the requirement that the attribute be a Date type. The rule can fail the second assert,
		which ensures there is no timezone info.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M409"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00275</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00275</xsl:attribute>
            <svrl:text>
		[ISM-ID-00275][Error] All @ism:declassDate attributes must be of type Date. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:declassDate attribute, this rule ensures that the declassDate value matches the pattern
		defined for type Date. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M410"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00276</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00276</xsl:attribute>
            <svrl:text>
		[ISM-ID-00276][Error] All @ism:declassEvent attributes must be a string with less than 1024 characters. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:declassEvent attribute, this rule ensures that the declassEvent value is a string with less
		than 1024 characters.   
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M411"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00277</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00277</xsl:attribute>
            <svrl:text>
		[ISM-ID-00277][Error] All @ism:declassException attributes must be of type NmTokens. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:declassException attribute, this rule ensures that the declassException value matches the pattern
		defined for type NmTokens. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M412"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00278</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00278</xsl:attribute>
            <svrl:text>
		[ISM-ID-00278][Error] All @ism:derivativelyClassifiedBy attributes must be a string with less than 1024 characters. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:derivativelyClassifiedBy attribute, 
	  	this rule ensures that the derivativelyClassifiedBy value is a string with less than 1024 characters.   
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M413"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00279</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00279</xsl:attribute>
            <svrl:text>
		[ISM-ID-00279][Error] All @ism:derivedFrom attributes must be a string with less than 1024 characters. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:derivedFrom attribute, this rule ensures that the derivedFrom value is a string with less
		than 1024 characters.   
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M414"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00280</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00280</xsl:attribute>
            <svrl:text>
		[ISM-ID-00280][Error] All @ism:displayOnlyTo attributes must be of type NmTokens. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:displayOnlyTo attribute, this rule ensures that the displayOnlyTo value matches the pattern
		defined for type NmTokens. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M415"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00281</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00281</xsl:attribute>
            <svrl:text>
		[ISM-ID-00281][Error] All @ism:disseminationControls attributes must be of type NmTokens. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain a @ism:disseminationControls attribute, the disseminationControls value must match the pattern
		defined for type NmTokens. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M416"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00283</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00283</xsl:attribute>
            <svrl:text>
		[ISM-ID-00283][Error] All @ism:FGIsourceOpen attributes must be of type NmTokens. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:FGIsourceOpen attribute, this rule ensures that the FGIsourceOpen value matches the pattern
		defined for type NmTokens. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M418"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00284</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00284</xsl:attribute>
            <svrl:text>
		[ISM-ID-00284][Error] All @ism:FGIsourceProtected attributes must be of type NmTokens. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:FGIsourceProtected attribute, this rule ensures that 
	  	the FGIsourceProtected value matches the pattern defined for type NmTokens. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M419"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00285</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00285</xsl:attribute>
            <svrl:text>
		[ISM-ID-00285][Error] All @ism:nonICmarkings attributes must be of type NmTokens. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:nonICmarkings attribute, this rule ensures that the nonICmarkings value matches the pattern
		defined for type NmTokens. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M420"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00286</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00286</xsl:attribute>
            <svrl:text>
		[ISM-ID-00286][Error] All @ism:nonUSControls attributes must be of type NmTokens. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:nonUSControls attribute, this rule ensures that the nonUSControls value matches the pattern
		defined for type NmTokens. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M421"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00287</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00287</xsl:attribute>
            <svrl:text>
		[ISM-ID-00287][Error] All @ism:noticeDate attributes must be of type Date. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:noticeDate attribute, this rule ensures that the noticeDate value matches the pattern
		defined for type Date. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M422"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00288</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00288</xsl:attribute>
            <svrl:text>
		[ISM-ID-00288][Error] All @ism:noticeReason attributes must be a string with less than 2048 characters. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:noticeReason attribute, this rule ensures that the noticeReason value is a string with less
		than 2048 characters.   
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M423"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00289</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00289</xsl:attribute>
            <svrl:text>
		[ISM-ID-00289][Error] All @ism:noticeType attributes must be of type NmTokens. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:noticeType attribute, this rule ensures that the noticeType value matches the pattern
		defined for type NmTokens. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M424"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00290</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00290</xsl:attribute>
            <svrl:text>
		[ISM-ID-00290][Error] All @ism:externalNotice attributes must be of type Boolean. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:externalNotice attribute, this rule ensures that the externalNotice value matches the pattern
		defined for type Boolean. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M425"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00291</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00291</xsl:attribute>
            <svrl:text>
		[ISM-ID-00291][Error] All @ism:ownerProducer attributes must be of type NmTokens. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:ownerProducer attribute, this rule ensures that the ownerProducer value matches the pattern
		defined for type NmTokens. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M426"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00292</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00292</xsl:attribute>
            <svrl:text>
		[ISM-ID-00292][Error] All @ism:pocType attributes must be of type NmTokens. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:pocType attribute, this rule ensures that the pocType value matches the pattern
		defined for type NmTokens. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M427"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00293</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00293</xsl:attribute>
            <svrl:text>
		[ISM-ID-00293][Error] All @ism:releasableTo attributes must be of type NmTokens. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:releasableTo attribute, this rule ensures that the releasableTo value matches the pattern
		defined for type NmTokens. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M428"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00294</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00294</xsl:attribute>
            <svrl:text>
	  	[ISM-ID-00294][Error] All @ism:resourceElement attributes must be of type Boolean. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:resourceElement attribute, this rule ensures that the resourceElement value matches the pattern
		defined for type Boolean. 
		
		Note: this rule is not able to be failed. If the resourceElement does
		not confirm to type Boolean, schematron fails when defining global
		variables before any rules are fired. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M429"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00295</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00295</xsl:attribute>
            <svrl:text>
		[ISM-ID-00295][Error] All @ism:SARIdentifier attributes must be of type NmTokens. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:SARIdentifier attribute, this rule ensures that the SARIdentifier value matches the pattern
		defined for type NmTokens. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M430"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00296</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00296</xsl:attribute>
            <svrl:text>
		[ISM-ID-00296][Error] All @ism:SCIcontrols attributes must be of type NmTokens. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:SCIcontrols attribute, this rule ensures that the SCIcontrols value matches the pattern
		defined for type NmTokens. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M431"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00297</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00297</xsl:attribute>
            <svrl:text>
		[ISM-ID-00297][Error] All @ism:unregisteredNoticeType attributes must be a string with less than 2048 characters. 
	</svrl:text>
            <svrl:text>
		For all elements which contain an @ism:unregisteredNoticeType attribute, this rule ensures that 
		the unregisteredNoticeType value is a string with less than 2048 characters.   
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M432"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00361</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00361</xsl:attribute>
            <svrl:text>
		[ISM-ID-00361][Error] All @ism:hasApproximateMarkings attributes must be of type Boolean. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:hasApproximateMarkings attribute, this rule ensures that the 
	  	hasApproximateMarkings value matches the pattern defined for type Boolean. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M471"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00365</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00365</xsl:attribute>
            <svrl:text>
        [ISM-ID-00365][Error] All @ism:noAggregation attributes must be of type Boolean. 
    </svrl:text>
            <svrl:text>
        For all elements which contain an @ism:noAggregation attribute, this rule ensures that the noAggregation value
        matches the pattern defined for type Boolean. 
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M475"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00379</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00379</xsl:attribute>
            <svrl:text>
        [ISM-ID-00379][Error] All ISM @ism:declassDate attributes must be a Date without a timezone.
    </svrl:text>
            <svrl:text>
        For all elements which contain a @ism:declassDate attribute, this rule ensures that
        the declassDate value matches the pattern defined for type Date without timezone information.
        The value must conform to the Regex ‘[0-9]{4}-[0-9]{2}-[0-9]{2}$’
    </svrl:text>
            <svrl:text>
        The first assert in this rule is not able to be failed in unit tests. If
        the declassDate does not conform to type Date, schematron fails when defining global
        variables before any rules are fired. The first assert is included as a normative statement
        of the requirement that the attribute be a Date type. The rule can fail the second assert,
        which ensures there is no timezone info.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M484"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00380</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00380</xsl:attribute>
            <svrl:text>
        [ISM-ID-00380][Error] All ISM @ism:noticeDate attributes must be a Date
        without a timezone.
    </svrl:text>
            <svrl:text>
        For all elements which contain a @ism:noticeDate attribute, this rule ensures that
        the noticeDate value matches the pattern defined for type Date without timezone information.
        The value must conform to the Regex ‘[0-9]{4}-[0-9]{2}-[0-9]{2}$’
    </svrl:text>
            <svrl:text>
        The first assert in this rule is not able to be failed in unit tests. If
        the noticeDate does not conform to type Date, schematron fails when defining global
        variables before any rules are fired. The first assert is included as a normative statement
        of the requirement that the attribute be a Date type. The rule can fail the second assert,
        which ensures there is no timezone info.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M485"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00484</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00484</xsl:attribute>
            <svrl:text>
		[ISM-ID-00484][Error] All @ism:cuiBasic attributes must be of type NmTokens. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:cuiBasic attribute, this rule ensures that the cuiBasic value matches the pattern
		defined for type NmTokens.  
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M527"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00485</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00485</xsl:attribute>
            <svrl:text>
		[ISM-ID-00485][Error] All @ism:cuiSpecified attributes must be of type NmTokens. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:cuiSpecified attribute, this rule ensures that the cuiSpecified value matches the pattern
		defined for type NmTokens.  
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M528"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00516</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00516</xsl:attribute>
            <svrl:text>
	  	[ISM-ID-00516][Error] All @ism:secondBannerLine attributes must be of type NmTokens. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain an @ism:secondBannerLine attribute, 
	  	this rule ensures that the secondBannerLine value matches the pattern defined for type NmTokens.  
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M552"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00340</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00340</xsl:attribute>
            <svrl:text>
		[ISM-ID-00340][Error] All @ism:compliesWith attributes must be of type NmTokens. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain a @ism:compliesWith attribute, this rule ensures that the @ism:compliesWith value 
	  	matches the pattern defined for type NmTokens. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M589"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00378</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00378</xsl:attribute>
            <svrl:text>
		[ISM-ID-00378][Error] All joint attributes must be of type Boolean. 
	</svrl:text>
            <svrl:text>
	  	For all elements which contain a @ism:joint attribute, this rule ensures that the joint value matches the pattern
		defined for type Boolean. 
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M596"/>
      </svrl:schematron-output>
   </xsl:template>

   <!--SCHEMATRON PATTERNS-->
<xsl:param name="countriesList"
              select="document('../../CVE/ISMCAT/CVEnumISMCATOwnerProducer.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="classificationAllList"
              select="document('../../CVE/ISM/CVEnumISMClassificationAll.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="classificationUSList"
              select="document('../../CVE/ISM/CVEnumISMClassificationUS.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="ownerProducerList"
              select="document('../../CVE/ISMCAT/CVEnumISMCATOwnerProducer.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="declassExceptionList"
              select="document('../../CVE/ISM/CVEnumISM25X.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="FGIsourceOpenList"
              select="document('../../CVE/ISMCAT/CVEnumISMCATFGIOpen.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="FGIsourceProtectedList"
              select="document('../../CVE/ISMCAT/CVEnumISMCATFGIProtected.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="nonICmarkingsList"
              select="document('../../CVE/ISM/CVEnumISMNonIC.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="releasableToList"
              select="document('../../CVE/ISMCAT/CVEnumISMCATRelTo.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="SCIcontrolsList"
              select="document('../../CVE/ISM/CVEnumISMSCIControls.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="SARIdentifierList"
              select="document('../../CVE/ISM/CVEnumISMSAR.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="SARSourceAuthorityList"
              select="document('../../CVE/ISM/CVEnumISMSARAuthorities.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="validAttributeList"
              select="document('../../CVE/ISM/CVEnumISMAttributes.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="validElementList"
              select="document('../../CVE/ISM/CVEnumISMElements.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="noticeList"
              select="document('../../CVE/ISM/CVEnumISMNotice.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="nonUSControlsList"
              select="document('../../CVE/ISM/CVEnumISMNonUSControls.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="exemptFromList"
              select="document('../../CVE/ISM/CVEnumISMExemptFrom.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="atomicEnergyMarkingsList"
              select="document('../../CVE/ISM/CVEnumISMAtomicEnergyMarkings.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="cuiBasicList"
              select="document('../../CVE/ISM/CVEnumISMCUIBasic.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="cuiSpecifiedList"
              select="document('../../CVE/ISM/CVEnumISMCUISpecified.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="secondBannerLineList"
              select="document('../../CVE/ISM/CVEnumISMSecondBannerLine.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="displayOnlyToList"
              select="document('../../CVE/ISMCAT/CVEnumISMCATRelTo.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="pocTypeList"
              select="document('../../CVE/ISM/CVEnumISMPocType.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="compliesWithList"
              select="document('../../CVE/ISM/CVEnumISMCompliesWith.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="accessPolicyList"
              select="document('../../CVE/ISM/CVEnumNTKAccessPolicy.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="profileDESList"
              select="document('../../CVE/ISM/CVEnumNTKProfileDes.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="licenseList"
              select="document('../../CVE/LIC/CVEnumLicLicense.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="usagencyList"
              select="document('../../CVE/USAgency/CVEnumUSAgencyAcronym.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="issueList"
              select="document('../../CVE/MN/CVEnumMNIssue.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="regionList"
              select="document('../../CVE/MN/CVEnumMNRegion.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="authcatList"
              select="document('../../CVE/AUTHCAT/CVEnumAuthCatType.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="entRoleValueList"
              select="document('../../CVE/ROLE/CVEnumROLEEnterpriseRole.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="NameStartCharPattern" select="':|[A-Z]|_|[a-z]'"/>
   <xsl:param name="NameCharPattern"
              select="concat($NameStartCharPattern, '|-|\.|[0-9]')"/>
   <xsl:param name="NmTokenPattern" select="concat('(', $NameCharPattern, ')+')"/>
   <xsl:param name="NmTokensPattern"
              select="concat($NmTokenPattern, '( ', $NmTokenPattern, ')*')"/>
   <xsl:param name="BooleanPattern" select="'(false|true|0|1)'"/>
   <xsl:param name="DatePattern"
              select="'-?([1-9][0-9]{3,}|0[0-9]{3})-(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])(Z|(\+|-)((0[0-9]|1[0-3]):[0-5][0-9]|14:00))?'"/>
   <xsl:param name="catRaw"
              select="document('../../Taxonomy/ISMCAT/TetragraphTaxonomy.xml')"/>
   <xsl:param name="catt"
              select="document('../../Taxonomy/ISMCAT/TetragraphTaxonomyDenormalized.xml')"/>
   <xsl:param name="cattMappings" select="$catt//catt:Tetragraph"/>
   <xsl:param name="tetragraphList"
              select="document('../../CVE/ISMCAT/CVEnumISMCATTetragraph.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="countriesAndTetras"
              select="          distinct-values(for $each in distinct-values((/descendant-or-self::node()//@ism:ownerProducer | /descendant-or-self::node()//@ism:releasableTo | /descendant-or-self::node()//@ism:displayOnlyTo | /descendant-or-self::node()//@ism:FGIsourceOpen | /descendant-or-self::node()//@ism:FGIsourceProtected))          return             util:tokenize($each))"/>
   <xsl:param name="tetras"
              select="          for $value in $countriesAndTetras          return             if ($catt//catt:Tetragraph[catt:TetraToken = $value]) then                $value             else                null"/>
   <xsl:param name="catt_new"
              select="          for $node in $catt//*          return             if (local-name($node) = 'Organization') then                'MEM'             else                $node"/>
   <xsl:param name="ISM_RESOURCE_ELEMENT"
              select="          (for $each in (//*)          return             if (if (string($each/@ism:resourceElement) castable as xs:boolean) then                if ($each/@ism:resourceElement = true()) then                   true()                else                   false()             else                false()) then                $each             else                null)[1]"/>
   <xsl:param name="ISM_RESOURCE_CREATE_DATE"
              select="$ISM_RESOURCE_ELEMENT/@ism:createDate"/>
   <xsl:param name="builtins"
              select="(('group:iaaems', 'JWICS:IAAEMS'), ('individual:icpki', 'IC-PKI:DN'), ('individual:cadpki', 'CAD-PKI:DN'), ('individual:acsspki', 'ACSS-PKI:DN'), ('organization:usa-agency', 'urn:us:gov:ic:cvenum:usagency:agencyacronym'), ('datasphere:license', 'urn:us:gov:ic:cvenum:lic:license'), ('datasphere:mn:issue', 'urn:us:gov:ic:cvenum:mn:issue'), ('datasphere:mn:region', 'urn:us:gov:ic:cvenum:mn:region'), ('datasphere:rac', 'urn:us:gov:ic:cvenum:authcat:authcattype', ('role:enterpriseRole', 'urn:us:gov:ic:cvenum:role:enterprise:role')))"/>
   <xsl:param name="builtinVocab"
              select="          for $each in $builtins[position() mod 2 eq 1]          return             $each"/>
   <xsl:param name="builtinVocabSource"
              select="          for $each in $builtins[position() mod 2 eq 0]          return             $each"/>
   <xsl:param name="ISM_USGOV_RESOURCE"
              select="util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:compliesWith, ('USGov'))"/>
   <xsl:param name="ISM_OTHER_AUTH_RESOURCE"
              select="util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:compliesWith, ('OtherAuthority'))"/>
   <xsl:param name="ISM_USIC_RESOURCE"
              select="util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:compliesWith, ('USIC'))"/>
   <xsl:param name="ISM_USDOD_RESOURCE"
              select="util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:compliesWith, ('USDOD'))"/>
   <xsl:param name="ISM_USCUI_RESOURCE"
              select="util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:compliesWith, ('USA-CUI'))"/>
   <xsl:param name="ISM_USCUIONLY_RESOURCE"
              select="util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:compliesWith, ('USA-CUI-ONLY'))"/>
   <xsl:param name="disseminationControlsList" select="util:getDissemControlsList()"/>
   <xsl:param name="ISM_710_FDR_EXEMPT"
              select="index-of(tokenize(normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:exemptFrom)), ' '), 'IC_710_MANDATORY_FDR') &gt; 0 or not($ISM_USIC_RESOURCE)"/>
   <xsl:param name="ISM_DOD_DISTRO_EXEMPT"
              select="index-of(tokenize(normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:exemptFrom)), ' '), 'DOD_DISTRO_STATEMENT') &gt; 0 or not($ISM_USDOD_RESOURCE)"/>
   <xsl:param name="ISM_ORCON_POC_DATE" select="xs:date('2011-03-11')"/>
   <xsl:param name="bannerClassification"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:classification))"/>
   <xsl:param name="bannerDisseminationControls"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:disseminationControls))"/>
   <xsl:param name="bannerDisplayOnlyTo"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:displayOnlyTo))"/>
   <xsl:param name="bannerNonICmarkings"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:nonICmarkings))"/>
   <xsl:param name="bannerFGIsourceOpen"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:FGIsourceOpen))"/>
   <xsl:param name="bannerFGIsourceProtected"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:FGIsourceProtected))"/>
   <xsl:param name="bannerReleasableTo"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:releasableTo))"/>
   <xsl:param name="bannerSCIcontrols"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:SCIcontrols))"/>
   <xsl:param name="bannerNotice"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:noticeType))"/>
   <xsl:param name="bannerSARIdentifier"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:SARIdentifier))"/>
   <xsl:param name="bannerAtomicEnergyMarkings"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings))"/>
   <xsl:param name="bannerCuiBasic"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:cuiBasic))"/>
   <xsl:param name="bannerCuiSpecified"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:cuiSpecified))"/>
   <xsl:param name="bannerDisseminationControls_tok"
              select="tokenize(normalize-space(string($bannerDisseminationControls)), ' ')"/>
   <xsl:param name="bannerDisplayOnlyTo_tok"
              select="tokenize(normalize-space(string($bannerDisplayOnlyTo)), ' ')"/>
   <xsl:param name="bannerNonICmarkings_tok"
              select="tokenize(normalize-space(string($bannerNonICmarkings)), ' ')"/>
   <xsl:param name="bannerFGIsourceOpen_tok"
              select="tokenize(normalize-space(string($bannerFGIsourceOpen)), ' ')"/>
   <xsl:param name="bannerFGIsourceProtected_tok"
              select="tokenize(normalize-space(string($bannerFGIsourceProtected)), ' ')"/>
   <xsl:param name="bannerReleasableTo_tok"
              select="tokenize(normalize-space(string($bannerReleasableTo)), ' ')"/>
   <xsl:param name="bannerSCIcontrols_tok"
              select="tokenize(normalize-space(string($bannerSCIcontrols)), ' ')"/>
   <xsl:param name="bannerNotice_tok"
              select="tokenize(normalize-space(string($bannerNotice)), ' ')"/>
   <xsl:param name="bannerSARIdentifier_tok"
              select="tokenize(normalize-space(string($bannerSARIdentifier)), ' ')"/>
   <xsl:param name="bannerAtomicEnergyMarkings_tok"
              select="tokenize(normalize-space(string($bannerAtomicEnergyMarkings)), ' ')"/>
   <xsl:param name="bannerCuiBasic_tok"
              select="tokenize(normalize-space(string($bannerCuiBasic)), ' ')"/>
   <xsl:param name="bannerCuiSpecified_tok"
              select="tokenize(normalize-space(string($bannerCuiSpecified)), ' ')"/>
   <xsl:param name="partTags"
              select="/descendant-or-self::node()[@ism:* except (@ism:pocType | @ism:DESVersion | @ism:unregisteredNoticeType | @ism:ISMCATCESVersion) and util:contributesToRollup(.) and not(generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT))]"/>
   <xsl:param name="partClassification"
              select="          for $token in $partTags/@ism:classification          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partOwnerProducer"
              select="          for $token in $partTags/@ism:ownerProducer          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partDisseminationControls"
              select="          for $token in $partTags/@ism:disseminationControls          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partDisplayOnlyTo"
              select="          for $token in $partTags/@ism:displayOnlyTo          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partAtomicEnergyMarkings"
              select="          for $token in $partTags/@ism:atomicEnergyMarkings          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partNonICmarkings"
              select="          for $token in $partTags/@ism:nonICmarkings          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partFGIsourceOpen"
              select="          for $token in $partTags/@ism:FGIsourceOpen          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partFGIsourceProtected"
              select="          for $token in $partTags/@ism:FGIsourceProtected          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partSCIcontrols"
              select="          for $token in $partTags/@ism:SCIcontrols          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partNoticeType"
              select="          for $token in $partTags/@ism:noticeType          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partSARIdentifier"
              select="          for $token in $partTags/@ism:SARIdentifier          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partCuiBasicTags"
              select="/descendant-or-self::node()[@ism:cuiBasic and util:contributesToRollup(.) and not(generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT))]"/>
   <xsl:param name="partCuiBasic"
              select="          for $token in $partCuiBasicTags/@ism:cuiBasic          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partCuiSpecifiedTags"
              select="/descendant-or-self::node()[@ism:cuiSpecified and util:contributesToRollup(.) and not(generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT))]"/>
   <xsl:param name="partCuiSpecified"
              select="          for $token in $partCuiSpecifiedTags/@ism:cuiSpecified          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partPocType"
              select="//*/@ism:pocType[util:contributesToRollup(./parent::node()) and not(generate-id(./parent::node()) = generate-id($ISM_RESOURCE_ELEMENT)) and not(./parent::node()/@ism:externalNotice = true())]"/>
   <xsl:param name="partClassification_tok"
              select="          for $token in $partClassification          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partOwnerProducer_tok"
              select="          for $token in $partOwnerProducer          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partDisseminationControls_tok"
              select="          for $token in $partDisseminationControls          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partDisplayOnlyTo_tok"
              select="          for $token in $partDisplayOnlyTo          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partAtomicEnergyMarkings_tok"
              select="          for $token in $partAtomicEnergyMarkings          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partNonICmarkings_tok"
              select="          for $token in $partNonICmarkings          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partSCIcontrols_tok"
              select="          for $token in $partSCIcontrols          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partNoticeType_tok"
              select="          for $token in $partNoticeType          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partSARIdentifier_tok"
              select="          for $token in $partSARIdentifier          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partPocType_tok"
              select="          for $token in $partPocType          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partCuiBasic_tok"
              select="          for $token in $partCuiBasic          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partCuiSpecified_tok"
              select="          for $token in $partCuiSpecified          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partNoticeNodeType"
              select="          for $token in $partTags/@ism:noticeType          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="ISM_NSI_EO_APPLIES"
              select="          $ISM_USGOV_RESOURCE and not($ISM_RESOURCE_ELEMENT/@ism:classification = 'U') and $ISM_RESOURCE_CREATE_DATE &gt;= xs:date('1996-04-11') and (some $element in $partTags             satisfies not($element/@ism:classification = 'U') and not($element/@ism:atomicEnergyMarkings))"/>
   <xsl:param name="dcTags"
              select="          for $piece in $disseminationControlsList          return             $piece/text()"/>
   <xsl:param name="dcTagsFound"
              select="          for $token in $dcTags          return             if (index-of($partDisseminationControls_tok, $token) &gt; 0 and (not(index-of($bannerDisseminationControls_tok, $token) &gt; 0))) then                $token             else                null"/>
   <xsl:param name="aeaTags"
              select="          for $piece in $atomicEnergyMarkingsList          return             $piece/text()"/>
   <xsl:param name="aeaTagsFound"
              select="          for $token in $aeaTags          return             if (index-of($partAtomicEnergyMarkings_tok, $token) &gt; 0 and (not(index-of($bannerAtomicEnergyMarkings_tok, $token) &gt; 0))) then                $token             else                null"/>
   <xsl:param name="ACCMRegex" select="'^ACCM-[A-Z0-9\-_]{1,61}$'"/>
   <xsl:param name="nonACCMLeftSet" select="'DS'"/>
   <xsl:param name="nonACCMRightSet" select="'XD,ND,SBU,SBU-NF,LES,LES-NF,SSI,NNPI'"/>
   <xsl:param name="nonACCMLeftSetTok" select="tokenize($nonACCMLeftSet, ',')"/>
   <xsl:param name="nonACCMRightSetTok" select="tokenize($nonACCMRightSet, ',')"/>
   <xsl:param name="decomposableTetraElems"
              select="$cattMappings[@decomposable[. = 'Yes' or . = 'NA']]"/>
   <xsl:param name="decomposableTetras"
              select="$decomposableTetraElems/catt:TetraToken/text()"/>
   <xsl:param name="countFdrPortions" select="count($partTags[util:containsFDR(.)])"/>
   <xsl:param name="relToCalculatedBannerTokens"
              select="util:calculateCommonCountries($partTags/@ism:releasableTo)"/>
   <xsl:param name="relToActualBannerTokens"
              select="util:expandDecomposableTetras($ISM_RESOURCE_ELEMENT/@ism:releasableTo)"/>
   <xsl:param name="displayToCalculatedBannerTokens"
              select="util:calculateCommonCountries(($partTags/@ism:releasableTo, $partTags/@ism:displayOnlyTo))"/>
   <xsl:param name="displayToActualBannerTokens"
              select="util:expandDecomposableTetras(util:join(($ISM_RESOURCE_ELEMENT/@ism:releasableTo, $ISM_RESOURCE_ELEMENT/@ism:displayOnlyTo)))"/>

   <!--PATTERN ISM-ID-00268-->


	<!--RULE ISM-ID-00268-R1-->
<xsl:template match="*[@ism:atomicEnergyMarkings]" priority="1000" mode="M403">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:atomicEnergyMarkings]"
                       id="ISM-ID-00268-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:atomicEnergyMarkings, $NmTokensPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:atomicEnergyMarkings, $NmTokensPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00268][Error] All @ism:atomicEnergyMarkings attributes must be of type NmTokens. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M403"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M403"/>
   <xsl:template match="@*|node()" priority="-2" mode="M403">
      <xsl:apply-templates select="*" mode="M403"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00269-->


	<!--RULE ISM-ID-00269-R1-->
<xsl:template match="*[@ism:classification]" priority="1000" mode="M404">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:classification]"
                       id="ISM-ID-00269-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:classification, $NmTokenPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:classification, $NmTokenPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00269][Error] All @ism:classification attributes must be of type NmToken. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M404"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M404"/>
   <xsl:template match="@*|node()" priority="-2" mode="M404">
      <xsl:apply-templates select="*" mode="M404"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00270-->


	<!--RULE ISM-ID-00270-R1-->
<xsl:template match="*[@ism:classificationReason]" priority="1000" mode="M405">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:classificationReason]"
                       id="ISM-ID-00270-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="string-length(@ism:classificationReason) &lt;= 4096"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="string-length(@ism:classificationReason) &lt;= 4096">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[ISM-ID-00270][Error] All @ism:classificationReason attributes must be a string with 4096 characters or less.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M405"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M405"/>
   <xsl:template match="@*|node()" priority="-2" mode="M405">
      <xsl:apply-templates select="*" mode="M405"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00271-->


	<!--RULE ISM-ID-00271-R1-->
<xsl:template match="*[@ism:classifiedBy]" priority="1000" mode="M406">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:classifiedBy]"
                       id="ISM-ID-00271-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="string-length(@ism:classifiedBy) &lt;= 1024"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="string-length(@ism:classifiedBy) &lt;= 1024">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00271][Error] All @ism:classifiedBy attributes must be a string with less than 1024 characters. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M406"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M406"/>
   <xsl:template match="@*|node()" priority="-2" mode="M406">
      <xsl:apply-templates select="*" mode="M406"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00272-->


	<!--RULE ISM-ID-00272-R1-->
<xsl:template match="*[@ism:compilationReason]" priority="1000" mode="M407">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:compilationReason]"
                       id="ISM-ID-00272-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="string-length(@ism:compilationReason) &lt;= 1024"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="string-length(@ism:compilationReason) &lt;= 1024">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00272][Error] All @ism:compilationReason attributes must be a string with less than 1024 characters. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M407"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M407"/>
   <xsl:template match="@*|node()" priority="-2" mode="M407">
      <xsl:apply-templates select="*" mode="M407"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00273-->


	<!--RULE ISM-ID-00273-R1-->
<xsl:template match="*[@ism:exemptFrom]" priority="1000" mode="M408">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:exemptFrom]"
                       id="ISM-ID-00273-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:exemptFrom, $NmTokensPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:exemptFrom, $NmTokensPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00273][Error] All @ism:exemptFrom attributes must be of type NmTokens. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M408"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M408"/>
   <xsl:template match="@*|node()" priority="-2" mode="M408">
      <xsl:apply-templates select="*" mode="M408"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00274-->


	<!--RULE ISM-ID-00274-R1-->
<xsl:template match="*[@ism:createDate]" priority="1000" mode="M409">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:createDate]"
                       id="ISM-ID-00274-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(string(@ism:createDate), $DatePattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(string(@ism:createDate), $DatePattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[ISM-ID-00274][Error] All @ism:createDate attribute values must be of type Date. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="matches(@ism:createDate, '[0-9]{4}-[0-9]{2}-[0-9]{2}$')"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="matches(@ism:createDate, '[0-9]{4}-[0-9]{2}-[0-9]{2}$')">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[ISM-ID-00274][Error] All @ism:createDate attribute values must not have any timezone information specified. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M409"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M409"/>
   <xsl:template match="@*|node()" priority="-2" mode="M409">
      <xsl:apply-templates select="*" mode="M409"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00275-->


	<!--RULE ISM-ID-00275-R1-->
<xsl:template match="*[@ism:declassDate]" priority="1000" mode="M410">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:declassDate]"
                       id="ISM-ID-00275-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(string(@ism:declassDate), $DatePattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(string(@ism:declassDate), $DatePattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00275][Error] All @ism:declassDate attributes must be of type Date.  
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M410"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M410"/>
   <xsl:template match="@*|node()" priority="-2" mode="M410">
      <xsl:apply-templates select="*" mode="M410"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00276-->


	<!--RULE ISM-ID-00276-R1-->
<xsl:template match="*[@ism:declassEvent]" priority="1000" mode="M411">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:declassEvent]"
                       id="ISM-ID-00276-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="string-length(@ism:declassEvent) &lt;= 1024"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="string-length(@ism:declassEvent) &lt;= 1024">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00276][Error] All @ism:declassEvent attributes must be a string with less than 1024 characters. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M411"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M411"/>
   <xsl:template match="@*|node()" priority="-2" mode="M411">
      <xsl:apply-templates select="*" mode="M411"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00277-->


	<!--RULE ISM-ID-00277-R1-->
<xsl:template match="*[@ism:declassException]" priority="1000" mode="M412">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:declassException]"
                       id="ISM-ID-00277-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:declassException, $NmTokensPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:declassException, $NmTokensPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00277][Error] All @ism:declassException attributes must be of type NmTokens. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M412"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M412"/>
   <xsl:template match="@*|node()" priority="-2" mode="M412">
      <xsl:apply-templates select="*" mode="M412"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00278-->


	<!--RULE ISM-ID-00278-R1-->
<xsl:template match="*[@ism:derivativelyClassifiedBy]"
                 priority="1000"
                 mode="M413">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:derivativelyClassifiedBy]"
                       id="ISM-ID-00278-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="string-length(@ism:derivativelyClassifiedBy) &lt;= 1024"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="string-length(@ism:derivativelyClassifiedBy) &lt;= 1024">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00278][Error] All @ism:derivativelyClassifiedBy attributes must be a string with less than 1024 characters. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M413"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M413"/>
   <xsl:template match="@*|node()" priority="-2" mode="M413">
      <xsl:apply-templates select="*" mode="M413"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00279-->


	<!--RULE ISM-ID-00279-R1-->
<xsl:template match="*[@ism:derivedFrom]" priority="1000" mode="M414">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:derivedFrom]"
                       id="ISM-ID-00279-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="string-length(@ism:derivedFrom) &lt;= 1024"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="string-length(@ism:derivedFrom) &lt;= 1024">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00279][Error] All @ism:derivedFrom attributes must be a string with less than 1024 characters. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M414"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M414"/>
   <xsl:template match="@*|node()" priority="-2" mode="M414">
      <xsl:apply-templates select="*" mode="M414"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00280-->


	<!--RULE ISM-ID-00280-R1-->
<xsl:template match="*[@ism:displayOnlyTo]" priority="1000" mode="M415">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:displayOnlyTo]"
                       id="ISM-ID-00280-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:displayOnlyTo, $NmTokensPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:displayOnlyTo, $NmTokensPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[ISM-ID-00280][Error] All @ism:displayOnlyTo attributes values must be of type NmTokens. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M415"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M415"/>
   <xsl:template match="@*|node()" priority="-2" mode="M415">
      <xsl:apply-templates select="*" mode="M415"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00281-->


	<!--RULE ISM-ID-00281-R1-->
<xsl:template match="*[@ism:disseminationControls]" priority="1000" mode="M416">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:disseminationControls]"
                       id="ISM-ID-00281-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:disseminationControls, $NmTokensPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:disseminationControls, $NmTokensPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00281][Error] All @ism:disseminationControls attributes must be of type NmTokens.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M416"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M416"/>
   <xsl:template match="@*|node()" priority="-2" mode="M416">
      <xsl:apply-templates select="*" mode="M416"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00283-->


	<!--RULE ISM-ID-00283-R1-->
<xsl:template match="*[@ism:FGIsourceOpen]" priority="1000" mode="M418">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:FGIsourceOpen]"
                       id="ISM-ID-00283-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:FGIsourceOpen, $NmTokensPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:FGIsourceOpen, $NmTokensPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00283][Error] All @ism:FGIsourceOpen attributes must be of type NmTokens.  
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M418"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M418"/>
   <xsl:template match="@*|node()" priority="-2" mode="M418">
      <xsl:apply-templates select="*" mode="M418"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00284-->


	<!--RULE ISM-ID-00284-R1-->
<xsl:template match="*[@ism:FGIsourceProtected]" priority="1000" mode="M419">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:FGIsourceProtected]"
                       id="ISM-ID-00284-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:FGIsourceProtected, $NmTokensPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:FGIsourceProtected, $NmTokensPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00284][Error] All @ism:FGIsourceProtected attributes must be of type NmTokens. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M419"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M419"/>
   <xsl:template match="@*|node()" priority="-2" mode="M419">
      <xsl:apply-templates select="*" mode="M419"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00285-->


	<!--RULE ISM-ID-00285-R1-->
<xsl:template match="*[@ism:nonICmarkings]" priority="1000" mode="M420">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:nonICmarkings]"
                       id="ISM-ID-00285-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:nonICmarkings, $NmTokensPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:nonICmarkings, $NmTokensPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00285][Error] All @ism:nonICmarkings attributes must be of type NmTokens.  
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M420"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M420"/>
   <xsl:template match="@*|node()" priority="-2" mode="M420">
      <xsl:apply-templates select="*" mode="M420"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00286-->


	<!--RULE ISM-ID-00286-R1-->
<xsl:template match="*[@ism:nonUSControls]" priority="1000" mode="M421">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:nonUSControls]"
                       id="ISM-ID-00286-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:nonUSControls, $NmTokensPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:nonUSControls, $NmTokensPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00286][Error] All @ism:nonUSControls attributes must be of type NmTokens. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M421"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M421"/>
   <xsl:template match="@*|node()" priority="-2" mode="M421">
      <xsl:apply-templates select="*" mode="M421"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00287-->


	<!--RULE ISM-ID-00287-R1-->
<xsl:template match="*[@ism:noticeDate]" priority="1000" mode="M422">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:noticeDate]"
                       id="ISM-ID-00287-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(string(@ism:noticeDate), $DatePattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(string(@ism:noticeDate), $DatePattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00287][Error] All @ism:noticeDate attributes must be of type Date. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M422"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M422"/>
   <xsl:template match="@*|node()" priority="-2" mode="M422">
      <xsl:apply-templates select="*" mode="M422"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00288-->


	<!--RULE ISM-ID-00288-R1-->
<xsl:template match="*[@ism:noticeReason]" priority="1000" mode="M423">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:noticeReason]"
                       id="ISM-ID-00288-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="string-length(@ism:noticeReason) &lt;= 2048"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="string-length(@ism:noticeReason) &lt;= 2048">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00288][Error] All @ism:noticeReason attributes must be a string with less than 2048 characters. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M423"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M423"/>
   <xsl:template match="@*|node()" priority="-2" mode="M423">
      <xsl:apply-templates select="*" mode="M423"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00289-->


	<!--RULE ISM-ID-00289-R1-->
<xsl:template match="*[@ism:noticeType]" priority="1000" mode="M424">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:noticeType]"
                       id="ISM-ID-00289-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:noticeType, $NmTokensPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:noticeType, $NmTokensPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[ISM-ID-00289][Error] All @ism:noticeType attributes values must be of type NmTokens. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M424"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M424"/>
   <xsl:template match="@*|node()" priority="-2" mode="M424">
      <xsl:apply-templates select="*" mode="M424"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00290-->


	<!--RULE ISM-ID-00290-R1-->
<xsl:template match="*[@ism:externalNotice]" priority="1000" mode="M425">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:externalNotice]"
                       id="ISM-ID-00290-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:externalNotice, $BooleanPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:externalNotice, $BooleanPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00290][Error] All @ism:externalNotice attributes must be of type Boolean. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M425"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M425"/>
   <xsl:template match="@*|node()" priority="-2" mode="M425">
      <xsl:apply-templates select="*" mode="M425"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00291-->


	<!--RULE ISM-ID-00291-R1-->
<xsl:template match="*[@ism:ownerProducer]" priority="1000" mode="M426">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:ownerProducer]"
                       id="ISM-ID-00291-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:ownerProducer, $NmTokensPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:ownerProducer, $NmTokensPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00291][Error] All @ism:ownerProducer attributes must be of type NmTokens. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M426"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M426"/>
   <xsl:template match="@*|node()" priority="-2" mode="M426">
      <xsl:apply-templates select="*" mode="M426"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00292-->


	<!--RULE ISM-ID-00292-R1-->
<xsl:template match="*[@ism:pocType]" priority="1000" mode="M427">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:pocType]"
                       id="ISM-ID-00292-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:pocType, $NmTokensPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:pocType, $NmTokensPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00292][Error] All @ism:pocType attributes must be of type NmTokens. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M427"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M427"/>
   <xsl:template match="@*|node()" priority="-2" mode="M427">
      <xsl:apply-templates select="*" mode="M427"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00293-->


	<!--RULE ISM-ID-00293-R1-->
<xsl:template match="*[@ism:releasableTo]" priority="1000" mode="M428">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:releasableTo]"
                       id="ISM-ID-00293-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:releasableTo, $NmTokensPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:releasableTo, $NmTokensPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00293][Error] All @ism:releasableTo attributes must be of type NmTokens. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M428"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M428"/>
   <xsl:template match="@*|node()" priority="-2" mode="M428">
      <xsl:apply-templates select="*" mode="M428"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00294-->


	<!--RULE ISM-ID-00294-R1-->
<xsl:template match="*[@ism:resourceElement]" priority="1000" mode="M429">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:resourceElement]"
                       id="ISM-ID-00294-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:resourceElement, $BooleanPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:resourceElement, $BooleanPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00294][Error] All @ism:resourceElement attributes must be of type Boolean. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M429"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M429"/>
   <xsl:template match="@*|node()" priority="-2" mode="M429">
      <xsl:apply-templates select="*" mode="M429"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00295-->


	<!--RULE ISM-ID-00295-R1-->
<xsl:template match="*[@ism:SARIdentifier]" priority="1000" mode="M430">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:SARIdentifier]"
                       id="ISM-ID-00295-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:SARIdentifier, $NmTokensPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:SARIdentifier, $NmTokensPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00295][Error] All @ism:SARIdentifier attributes must be of type NmTokens. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M430"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M430"/>
   <xsl:template match="@*|node()" priority="-2" mode="M430">
      <xsl:apply-templates select="*" mode="M430"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00296-->


	<!--RULE ISM-ID-00296-R1-->
<xsl:template match="*[@ism:SCIcontrols]" priority="1000" mode="M431">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:SCIcontrols]"
                       id="ISM-ID-00296-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:SCIcontrols, $NmTokensPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:SCIcontrols, $NmTokensPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00296][Error] All @ism:SCIcontrols attributes must be of type NmTokens.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M431"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M431"/>
   <xsl:template match="@*|node()" priority="-2" mode="M431">
      <xsl:apply-templates select="*" mode="M431"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00297-->


	<!--RULE ISM-ID-00297-R1-->
<xsl:template match="*[@ism:unregisteredNoticeType]" priority="1000" mode="M432">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:unregisteredNoticeType]"
                       id="ISM-ID-00297-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="string-length(@ism:unregisteredNoticeType) &lt;= 2048"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="string-length(@ism:unregisteredNoticeType) &lt;= 2048">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00297][Error] All @ism:unregisteredNoticeType attributes must be a string with less than 2048 characters.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M432"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M432"/>
   <xsl:template match="@*|node()" priority="-2" mode="M432">
      <xsl:apply-templates select="*" mode="M432"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00361-->


	<!--RULE ISM-ID-00361-R1-->
<xsl:template match="*[@ism:hasApproximateMarkings]" priority="1000" mode="M471">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:hasApproximateMarkings]"
                       id="ISM-ID-00361-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:hasApproximateMarkings, $BooleanPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:hasApproximateMarkings, $BooleanPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00361][Error] All @ism:hasApproximateMarkings attributes values must be of type Boolean. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M471"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M471"/>
   <xsl:template match="@*|node()" priority="-2" mode="M471">
      <xsl:apply-templates select="*" mode="M471"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00365-->


	<!--RULE ISM-ID-00365-R1-->
<xsl:template match="*[@ism:noAggregation]" priority="1000" mode="M475">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:noAggregation]"
                       id="ISM-ID-00365-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:noAggregation, $BooleanPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:noAggregation, $BooleanPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00365][Error] All @ism:noAggregation attributes must be of type Boolean.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M475"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M475"/>
   <xsl:template match="@*|node()" priority="-2" mode="M475">
      <xsl:apply-templates select="*" mode="M475"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00379-->


	<!--RULE ISM-ID-00379-R1-->
<xsl:template match="*[@ism:declassDate]" priority="1000" mode="M484">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:declassDate]"
                       id="ISM-ID-00379-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:declassDate, $DatePattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:declassDate, $DatePattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00379][Error] All @ism:declassDate attribute values must be of type Date. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="matches(string(@ism:declassDate), '[0-9]{4}-[0-9]{2}-[0-9]{2}$')"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="matches(string(@ism:declassDate), '[0-9]{4}-[0-9]{2}-[0-9]{2}$')">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00379][Error] All @ism:declassDate attribute values must not have any timezone
            information specified. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M484"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M484"/>
   <xsl:template match="@*|node()" priority="-2" mode="M484">
      <xsl:apply-templates select="*" mode="M484"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00380-->


	<!--RULE ISM-ID-00380-R1-->
<xsl:template match="*[@ism:noticeDate]" priority="1000" mode="M485">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:noticeDate]"
                       id="ISM-ID-00380-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(string(@ism:noticeDate), $DatePattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(string(@ism:noticeDate), $DatePattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00380][Error] All @ism:noticeDate attribute values must be of type Date. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="matches(@ism:noticeDate, '[0-9]{4}-[0-9]{2}-[0-9]{2}$')"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="matches(@ism:noticeDate, '[0-9]{4}-[0-9]{2}-[0-9]{2}$')">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00380][Error] All @ism:noticeDate attribute values must not have any timezone
            information specified. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M485"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M485"/>
   <xsl:template match="@*|node()" priority="-2" mode="M485">
      <xsl:apply-templates select="*" mode="M485"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00484-->


	<!--RULE ISM-ID-00484-R1-->
<xsl:template match="*[@ism:cuiBasic]" priority="1000" mode="M527">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:cuiBasic]"
                       id="ISM-ID-00484-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:cuiBasic, $NmTokensPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:cuiBasic, $NmTokensPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00484][Error]  All @ism:cuiBasic attributes must be of type NmTokens.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M527"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M527"/>
   <xsl:template match="@*|node()" priority="-2" mode="M527">
      <xsl:apply-templates select="*" mode="M527"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00485-->


	<!--RULE ISM-ID-00485-R1-->
<xsl:template match="*[@ism:cuiSpecified]" priority="1000" mode="M528">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:cuiSpecified]"
                       id="ISM-ID-00485-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:cuiSpecified, $NmTokensPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:cuiSpecified, $NmTokensPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00485][Error] All @ism:cuiSpecified attributes must be of type NmTokens. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M528"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M528"/>
   <xsl:template match="@*|node()" priority="-2" mode="M528">
      <xsl:apply-templates select="*" mode="M528"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00516-->


	<!--RULE ISM-ID-00516-R1-->
<xsl:template match="*[@ism:secondBannerLine]" priority="1000" mode="M552">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:secondBannerLine]"
                       id="ISM-ID-00516-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:secondBannerLine, $NmTokensPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:secondBannerLine, $NmTokensPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[ISM-ID-00516][Error] All @ism:secondBannerLine attributes must be of type NmTokens. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M552"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M552"/>
   <xsl:template match="@*|node()" priority="-2" mode="M552">
      <xsl:apply-templates select="*" mode="M552"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00340-->


	<!--RULE ISM-ID-00340-R1-->
<xsl:template match="*[@ism:compliesWith]" priority="1000" mode="M589">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:compliesWith]"
                       id="ISM-ID-00340-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:compliesWith, $NmTokensPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:compliesWith, $NmTokensPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00340][Error] All @ism:compliesWith attributes must be of type NmTokens. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M589"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M589"/>
   <xsl:template match="@*|node()" priority="-2" mode="M589">
      <xsl:apply-templates select="*" mode="M589"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00378-->


	<!--RULE ISM-ID-00378-R1-->
<xsl:template match="*[@ism:joint]" priority="1000" mode="M596">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:joint]"
                       id="ISM-ID-00378-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:meetsType(@ism:joint, $BooleanPattern)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:meetsType(@ism:joint, $BooleanPattern)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[ISM-ID-00378][Error] All joint attributes values must be of type Boolean. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M596"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M596"/>
   <xsl:template match="@*|node()" priority="-2" mode="M596">
      <xsl:apply-templates select="*" mode="M596"/>
   </xsl:template>
</xsl:stylesheet>
<!--UNCLASSIFIED-->
