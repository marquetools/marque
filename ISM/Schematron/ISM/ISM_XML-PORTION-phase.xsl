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
         <xsl:attribute name="phase">PORTION</xsl:attribute>
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
            <xsl:attribute name="id">ISM-ID-00157</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00157</xsl:attribute>
            <svrl:text>
        [ISM-ID-00157][Error] If ISM_USDOD_RESOURCE and: 
        1. The attribute notice contains one of the [DoD-Dist-B], [DoD-Dist-C], [DoD-Dist-D], or [DoD-Dist-E] 
          AND
        2. The attribute @ism:noticeReason is not specified. 
        
        Human Readable: DoD distribution statements B, C, D , or E all require a reason. 
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USDOD_RESOURCE, for each element which
        specifies attribute ism:noticeType with a value containing the token [DoD-Dist-B],
        [DoD-Dist-C], [DoD-Dist-D], or [DoD-Dist-E], this rule ensures that attribute
        @ism:noticeReason is specified. 
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M246"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00161</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00161</xsl:attribute>
            <svrl:text>
        [ISM-ID-00161][Error] If the document is an
        1. ISM_USDOD_RESOURCE AND
        2. the attribute @ism:noticeType of ISM_RESOURCE_ELEMENT contains [DoD-Dist-A] AND
        3. no portions in the document have their attribute @ism:excludeFromRollup set to [true]
        THEN there must not be any attribute @ism:nonICmarkings present.
        
        Human Readable: Distribution statement A (Public Release) is 
        incompatible with any nonICMarkings if excludeFromRollup is not TRUE.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USDOD_RESOURCE and @ism:noticeType contains 'DoD-Dist-A' 
        and no portions in the document have their @ism:excludeFromRollup set to true, 
        then there must not be any @ism:nonICMarkings present.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M248"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00237</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00237</xsl:attribute>
            <svrl:text>
        [ISM-ID-00237][Error] If ISM_USDOD_RESOURCE, any element which specifies
        attribute @ism:noticeType containing one of the tokens [DoD-Dist-B], 
       	[DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F]
       	must also specify attribute @ism:noticeDate.     	
        
        Human Readable: DoD distribution statements B, C, D, E, and F all require a date.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:noticeType specified with a value containing the token
        [DoD-Dist-B], [DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F], 
        this rule ensures that attribute @ism:noticeDate is specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M251"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00238</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00238</xsl:attribute>
            <svrl:text>
    	[ISM-ID-00238][Error] If ISM_USDOD_RESOURCE, if any element specifies
    	attribute @ism:noticeType containing one of the tokens [DoD-Dist-B], 
    	[DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F]
    	then an element in the document must specify attribute @ism:pocType with
    	the same value as attribute @ism:noticeType.
        
        Human Readable: DoD distribution statements B, C, D, E, and F all 
        require a corresponding point of contact.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USDOD_RESOURCE, for each element which has 
    	attribute @ism:noticeType specified with a value containing the token
        [DoD-Dist-B], [DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F], 
        this rule ensures that some element in the document 
        specifies attribute @ism:pocType with the same value as @ism:noticeType.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M252"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00016</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00016</xsl:attribute>
            <svrl:text>
        [ISM-ID-00016][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:classification has a value of [U], then attributes @ism:classificationReason,
        @ism:classifiedBy, @ism:derivativelyClassifiedBy, @ism:declassDate, @ism:declassEvent, 
        @ism:declassException, @ism:derivedFrom, @ism:SARIdentifier, or @ism:SCIcontrols must not be specified.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:classification specified with a value of [U] this rule ensures that NONE of the following attributes 
    	are specified: @ism:classifiedBy, @ism:declassDate, @ism:declassEvent, @ism:declassException,
    	@ism:derivativelyClassifiedBy, @ism:derivedFrom, @ism:SARIdentifier, or @ism:SCIcontrols. 
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M257"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00017</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00017</xsl:attribute>
            <svrl:text>
        [ISM-ID-00017][Error] If ISM_NSI_EO_APPLIES and attribute 
        @ism:classifiedBy is specified, then attribute @ism:classificationReason must be specified.         
        Human Readable: Documents under E.O. 13526 containing Originally Classified data require a
        classification reason to be identified.
    </svrl:text>
            <svrl:text>
    	If ISM_NSI_EO_APPLIES, for each element which specifies attribute @ism:classifiedBy, 
    	this rule ensures that attribute @ism:classificationReason is specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M258"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00028</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00028</xsl:attribute>
            <svrl:text>
      [ISM-ID-00028][Error] If ISM_USGOV_RESOURCE and attribute 
      @ism:disseminationControls contains the name token [OC] or [EYES],
      then attribute @ism:classification must have a value of [TS], [S], or [C].
      Human Readable: Portions marked for ORCON or EYES ONLY dissemination 
      in a USA document must be CONFIDENTIAL, SECRET, or TOP SECRET.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [OC] or [EYES] this rule ensures that attribute
    	@ism:classification is specified with a value of [C], [S], or [TS].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M259"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00030</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00030</xsl:attribute>
            <svrl:text>
        [ISM-ID-00030][Error] If ISM_USGOV_RESOURCE and attribute @ism:disseminationControls contains the name token [FOUO], 
        then attribute @ism:classification must have a value of [U].
        Human Readable: Portions marked for FOUO dissemination in a USA document
        must be classified UNCLASSIFIED.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [FOUO] this rule ensures that attribute @ism:classification is 
    	specified with a value of [U].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M260"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00031</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00031</xsl:attribute>
            <svrl:text>
        [ISM-ID-00031][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:disseminationControls contains the name token [REL] or [EYES], then 
        attribute @ism:releasableTo must be specified. 
        Human Readable: USA documents containing REL TO or EYES ONLY 
        dissemination must specify to which countries the document is releasable.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [REL] or [EYES] this rule ensures that attribute @ism:releasableTo
    	is specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M261"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00032</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00032</xsl:attribute>
            <svrl:text>
        [ISM-ID-00032][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:disseminationControls is not specified, or is specified and does not 
        contain the name token [REL] or [EYES], then attribute @ism:releasableTo 
        must not be specified.
        
        Human Readable: USA documents must only specify to which countries it is 
        authorized for release if dissemination information contains 
        REL TO or EYES ONLY data. 
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        does not specify attribute @ism:disseminationControls or specifies attribute
        @ism:disseminationControls with a value containing the token 
        [REL] or [EYES] this rule ensures that attribute @ism:releasableTo is not 
        specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M262"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00033</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00033</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that mutually exclusive tokens do not exist in
		an attribute. The calling rule must pass @ism:disseminationControls and ('REL', 'EYES', 'NF').</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M263"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00038</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00038</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that mutually exclusive tokens do not exist in
		an attribute. The calling rule must pass @ism:nonICmarkings and ('XD', 'ND', 'SBU', 'SBU-NF').</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M265"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00040</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00040</xsl:attribute>
            <svrl:text>This abstract pattern checks to see if an attribute of an element exists
        in a list. The calling rule must pass *[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:ownerProducer, ('USA'))], @ism:classification, $classificationUSList, '   [ISM-ID-00040][Error] If ISM_USGOV_RESOURCE and attribute ownerProducer contains [USA] then attribute classification must have a value in CVEnumISMClassificationUS.xml.   '.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M266"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00043</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00043</xsl:attribute>
            <svrl:text>
        [ISM-ID-00043][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains the name token [SI], then attribute @ism:classification must have
        a value of [TS], [S], or [C].
        
        Human Readable: A USA document containing Special Intelligence (SI) 
        data must be classified CONFIDENTIAL, SECRET, or TOP SECRET.  
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [SI] this rule ensures that attribute @ism:classification is specified with
        a value containing the token [TS], [S], or [C].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M267"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00044</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00044</xsl:attribute>
            <svrl:text>
        [ISM-ID-00044][Error] If the document is an ISM_USGOV_RESOURCE and the
        attribute @ism:SCIcontrols contain a name token with [SI-G], then the attribute @ism:classification
        must have a value of [TS]. 
        
        Human Readable: A USA document containing Special Intelligence (SI) GAMMA compartment data 
        must be classified TOP SECRET. 
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing a token with [SI-G] this rule
        ensures that attribute @ism:classification is specified with a value containing the token [TS].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M268"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00045</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00045</xsl:attribute>
            <svrl:text>
        [ISM-ID-00045][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains a name token starting with [SI-G], then attribute
        @ism:disseminationControls must contain the name token [OC].
        
        Human Readable: A USA document containing Special Intelligence (SI)
        GAMMA compartment data must be marked for ORIGINATOR CONTROLLED 
        dissemination.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing a token
        starting with [SI-G] this rule ensures that attribute
        @ism:disseminationControls is specified with a value containing the
        token [OC].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M269"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00047</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00047</xsl:attribute>
            <svrl:text>
        [ISM-ID-00047][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains the name token [TK], then attribute @ism:classification must have
        a value of [TS] or [S].
        
        Human Readable: A USA document containing TALENT KEYHOLE data must
        be classified SECRET or TOP SECRET.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [TK] this rule ensures that attribute @ism:classification is 
        specified with a value containing the token [TS] or [S].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M270"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00048</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00048</xsl:attribute>
            <svrl:text>
        [ISM-ID-00048][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains the name token [HCS], then attribute @ism:classification must have
        a value of [TS] or [S].
        
        Human Readable: A USA document containing HCS data must be classified
        SECRET or TOP SECRET.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [HCS] this rule ensures that attribute @ism:classification is 
        specified with a value containing the token [TS] or [S].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M271"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00049</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00049</xsl:attribute>
            <svrl:text>
        [ISM-ID-00049][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains the name token [HCS], then attribute @ism:disseminationControls
        must contain the name token [NF].
        
        Human Readable: A USA document containing HCS data must be marked
        for NO FOREIGN dissemination.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [HCS] this rule ensures that attribute @ism:disseminationControls is 
        specified with a value containing the token [NF].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M272"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00097</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00097</xsl:attribute>
            <svrl:text>
        [ISM-ID-00097][Warning] If ISM_USGOV_RESOURCE and attribute @ism:FGIsourceProtected is 
        specified with a value other than [FGI] then the value(s) must not be discoverable in IC shared spaces.
        
        Human Readable: FGI Protected should rarely if ever be seen outside of an agency's internal systems.    
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE, for each element which specifies
    	the attribute @ism:FGIsourceProtected, this rule ensures that attribute
    	@ism:FGIsourceProtected contains only the token [FGI].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M298"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00099</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00099</xsl:attribute>
            <svrl:text>
        [ISM-ID-00099][Error] If ISM_USGOV_RESOURCE and attribute @ism:ownerProducer
        contains the token [FGI], then the token [FGI] must be the only value in attribute @ism:ownerProducer.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribtue @ism:ownerProducer with a value containing the token
        [FGI] this rule ensures that attribute @ism:ownerProducer only contains a 
        single token.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M299"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00107</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00107</xsl:attribute>
            <svrl:text>
        [ISM-ID-00107][Error] If ISM_USGOV_RESOURCE and attribute @ism:disseminationControls contains the name token [IMC] 
        then attribute @ism:classification must have a value of [TS] or [S].
        
        Human Readable: IMCON data is SECRET (S), but may appear with S or TOP SECRET data.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [IMC] this rule ensures that attribute @ism:classification is not
    	specified with a value of [TS] or [S].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M302"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00124</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00124</xsl:attribute>
            <svrl:text>
      [ISM-ID-00124][Warning] If ISM_USGOV_RESOURCE and
      1. Attribute @ism:ownerProducer does not contain [USA].
      AND
      2. Attribute @ism:disseminationControls contains [RELIDO]
      
      Human Readable: RELIDO is not authorized for non-US portions.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [RELIDO] this rule ensures that attribute @ism:ownerProducer is
    	specified with a value containing [USA].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M306"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00127</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00127</xsl:attribute>
            <svrl:text>
		Abstract pattern to enforce that an appropriate notice exists for an
		element in $partTags that has a notice requirement. The calling rule must pass $elem,
		'atomicEnergyMarkings', $partTags, and 'RD'.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M307"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00128</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00128</xsl:attribute>
            <svrl:text>
		For all elements that contribute to rollup when all of the following are true:
		(a) the given expression $ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings contains the given value 'FRD'
		(b) the given exception expression $ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings does not contain the given exception value 'RD'
		(c) $ISM_USGOV_RESOURCE is true
		
		Assert that some non-resource node element satisfies both
		(a) @ism:noticeType contains the 'FRD' token
		(b) not(@ism:externalNotice is true)

		This rule depends on $partTags defined in the ISM_XML.sch master Schematron file.
		
		The calling rule must pass $ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings, 'FRD', $ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings, 'RD'.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M308"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00129</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00129</xsl:attribute>
            <svrl:text>
		Abstract pattern to enforce that an appropriate notice exists for an
		element in $partTags that has a notice requirement. The calling rule must pass $elem,
		'disseminationControls', $partTags, and 'IMC', 'IMCON_RSEN'.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M309"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00130</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00130</xsl:attribute>
            <svrl:text>
		Abstract pattern to enforce that an appropriate notice exists for an
		element in $partTags that has a notice requirement. The calling rule must pass $elem,
		'disseminationControls', $partTags, and 'FISA'.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M310"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00133</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00133</xsl:attribute>
            <svrl:text>
		[ISM-ID-00133][Error] If ISM_NSI_EO_APPLIES and attribute 
		@ism:declassException is specified and contains the tokens [25X1-EO-12951],
		[50X1-HUM], [50X2-WMD], [NATO], [AEA] or [NATO-AEA] 
		then attribute @ism:declassDate or @ism:declassEvent must NOT be specified.
		
		Human Readable: Documents under E.O. 13526 must not specify declassDate or declassEvent if 
		a declassException of 25X1-EO-12951, 50X1-HUM, 50X2-WMD, NATO, AEA or NATO-AEA is specified.
	</svrl:text>
            <svrl:text>
		If ISM_NSI_EO_APPLIES, for each element which specifies 
		@ism:declassException with a value containing token [25X1-EO-12951], [50X1-HUM], [50X2-WMD], [NATO], [AEA] 
		or [NATO-AEA] this rule ensures that attributes @ism:declassDate and @ism:declassEvent are NOT specified.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M312"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00134</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00134</xsl:attribute>
            <svrl:text>
		Abstract pattern to enforce that an appropriate notice exists for an
		element in $partTags that has a notice requirement. The calling rule must pass $elem,
		'nonICmarkings', $partTags, and 'DS'.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M313"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00135</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00135</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that for a given element in a document that is
		ISM_USGOV_RESOURCE with no CUI, with @ism:noticeType containing a specified token and @ism:externalNotice
		not equal true, 'RD' exists in $partAtomicEnergyMarkings_tok. The calling rule must pass 'RD',
		'RD' and @dataTokenList.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M314"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00136</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00136</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that for a given element in a document that is
		ISM_USGOV_RESOURCE with no CUI, with @ism:noticeType containing a specified token and @ism:externalNotice
		not equal true, 'FRD' exists in $partAtomicEnergyMarkings_tok. The calling rule must pass 'FRD',
		'FRD' and @dataTokenList.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M315"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00138</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00138</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that for a given element in an
		ISM_USGOV_RESOURCE with @ism:noticeType containing a specified token and @ism:externalNotice
		not equal true, 'DS' exists in $partNonICmarkings_tok ONLY if the $ISM_RESOURCE_ELEMENT is Unclassified.
		The calling rule must pass 'DS' and @dataTokenList.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M316"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00139</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00139</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that for a given element in a document that is
		ISM_USGOV_RESOURCE with no CUI, with @ism:noticeType containing a specified token and @ism:externalNotice
		not equal true, 'FISA' exists in ($partCuiSpecified_tok,$partDisseminationControls_tok). The calling rule must pass 'FISA',
		'FISA' and @dataTokenList.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M317"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00143</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00143</xsl:attribute>
            <svrl:text>
        [ISM-ID-00143][Error] If ISM_USGOV_RESOURCE and attribute @ism:derivativelyClassifiedBy is specified, 
        then attribute @ism:derivedFrom must be specified. 
        
        Human Readable: Derivatively Classified data including DOE data requires
        a derived from value to be identified.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE, for each element which 
    	specifies attribute @ism:derivativelyClassifiedBy this rule ensures that
    	attribute @ism:derivedFrom is specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M320"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00148</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00148</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that mutually exclusive tokens do not exist in
		an attribute. The calling rule must pass @ism:nonICmarkings and ('LES', 'LES-NF').</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M324"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00150</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00150</xsl:attribute>
            <svrl:text>
    [ISM-ID-00150][Error] If (ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE) and:
    1. Any element, other than ISM_RESOURCE_ELEMENT, meeting ISM_CONTRIBUTES in the document has the 
    attribute @ism:nonICmarkings containing [LES] or the attribute @ism:cuiBasic containing [LEI]
    AND
    2. No element meeting ISM_CONTRIBUTES in the document has the attribute @ism:noticeType containing [LES]
    
    Human Readable: USA documents containing LES non-IC markings or LEI cuiBasic markings must also have an 
    LES notice.
  </svrl:text>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE, for each element which
    is not the ISM_RESOURCE_ELEMENT and meets ISM_CONTRIBUTES and specifies 
    attribute @ism:nonICmarkings with a value containing the token [LES]
    or @ism:cuiBasic with a value containing the token [LEI], 
    this rule ensures that an element meeting ISM_CONTRIBUTES specifies attribute
    @ism:noticeType with a value containing the token [LES].
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M326"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00151</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00151</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that for a given element in a document that is
		ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE, with @ism:noticeType containing a specified token and @ism:externalNotice
		not equal true, either $dataType or 'LEI' exists in ($partNonICmarkings_tok,$partCuiBasic_tok). The calling rule must pass 'LES',
		'LEI', 'LES', 'nonICmarkings or cuiBasic' and ($partNonICmarkings_tok,$partCuiBasic_tok).  This rule was created because the token for Law Enforcement data
	    is [LES] in @ism:disseminationControls and [LEI] in @ism:cuiBasic, but both tokens require an [LES] notice.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M327"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00152</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00152</xsl:attribute>
            <svrl:text>
		Abstract pattern to enforce that an appropriate notice exists for an
		element in $partTags that has a notice requirement. The calling rule must pass $elem,
		'nonICmarkings', $partTags, and 'LES-NF'.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M328"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00153</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00153</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that for a given element in a document that is
		ISM_USGOV_RESOURCE with no CUI, with @ism:noticeType containing a specified token and @ism:externalNotice
		not equal true, 'LES-NF' exists in $partNonICmarkings_tok. The calling rule must pass 'LES-NF',
		'LES-NF' and @dataTokenList.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M329"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00159</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00159</xsl:attribute>
            <svrl:text>
        [ISM-ID-00159][Error] If ISM_USGOV_RESOURCE and:
        1. attribute @ism:classification of ISM_RESOURCE_ELEMENT is not [U]
        AND
        2. The attribute @ism:noticeType does contain [DoD-Dist-A] or has attribute @ism:externalNotice with a value of [true].
        
        Human Readable: Distribution statement A (Public Release) is forbidden on classified documents.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE and the attribute
        @ism:classification of ISM_RESOURCE_ELEMENT is not [U], for each element
        which specifies attribute @ism:noticeType this rule ensures that attribute
        @ism:noticeType is not specified with a value containing the token
        [DoD-Dist-A] unless it is an external notice with attribute @ism:externalNotice is [true].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M331"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00164</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00164</xsl:attribute>
            <svrl:text>
        [ISM-ID-00164][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:disseminationControls contains the name token [RS],
        then attribute @ism:classification must have a value of [TS] or [S].
        
        Human Readable: USA documents with RISK SENSITIVE dissemination must
        be classified SECRET or TOP SECRET.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [RS] this rule ensures that attribute @ism:classification is not
    	specified with a value of [TS] or [S].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M332"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00166</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00166</xsl:attribute>
            <svrl:text>
		Abstract pattern used to warn that an attribute value has a deprecation
		date in its CVE but has not passed based on the ISM_RESOURCE_CREATE_DATE of the resource.
		This pattern uses the deprecation dates in the CVE passed from the calling rule and the
		ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute has a depreciation date,
		which is a warning. The context, CVE name, and Spec name are passed from the calling
		rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M334"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00168</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00168</xsl:attribute>
            <svrl:text>
        [ISM-ID-00168][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:disseminationControls is not specified or is specified and does not contain the name token 
        [DISPLAYONLY], then attribute @ism:displayOnlyTo must not be specified.
        
        Human Readable: If a portion in a USA document is not marked for DISPLAY ONLY dissemination, 
        it must not list countries to which it may be disclosed. 
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE and attribute @ism:disseminationControls
        does not contain the token [DISPLAYONLY], this rule ensures that the attribute 
      	@ism:displayOnlyTo is not specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M335"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00169</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00169</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that mutually exclusive tokens do not exist in
		an attribute. The calling rule must pass @ism:disseminationControls and ('DISPLAYONLY', 'RELIDO', 'NF').</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M336"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00170</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00170</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that an attribute does not contain a deprecated token. 
		This pattern uses the deprecation dates in the CVE passed from the calling rule and 
		the ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute is deprecated, 
		which is an error. The context, CVE name, and Spec name are passed from the calling rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M337"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00173</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00173</xsl:attribute>
            <svrl:text>
        [ISM-ID-00173][Error] If ISM_USGOV_RESOURCE and attribute
        @ism:atomicEnergyMarkings contains a name token starting with [RD-SG] or [FRD-SG], then attribute
        @ism:classification must have a value of [S] or [TS]. 
        
        Human Readable: Portions in a USA document that contain RD or FRD SIGMA data must be marked SECRET or TOP SECRET. 
    </svrl:text>
            <svrl:text>
	      If the document is an ISM_USGOV_RESOURCE, for each element which has
        attribute @ism:atomicEnergyMarkings specified with a value containing a token starting with
        [RD-SG] or [FRD-SG], this rule ensures that the attribute @ism:classification has a value of [S] or [TS]. 
	  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M338"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00174</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00174</xsl:attribute>
            <svrl:text>
        [ISM-ID-00174][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:atomicEnergyMarkings contains the name token [RD], [FRD], or [TFNI], 
        then attribute @ism:classification must have a value of [TS], [S], or [C].
        
        Human Readable: USA documents with RD, FRD, or TFNI data must be marked CONFIDENTIAL,
        SECRET, or TOP SECRET.
    </svrl:text>
            <svrl:text>
		If the document is an ISM_USGOV_RESOURCE, for each element which has 
		attribute @ism:atomicEnergyMarkings specified with a value containing 
		the token [RD], [FRD], or [TFNI], this rule ensures that the attribute 
		@ism:classification has a value of [TS], [S], or [C].
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M339"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00175</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00175</xsl:attribute>
            <svrl:text>
        [ISM-ID-00175][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:atomicEnergyMarkings contains the name token [RD-CNWDI], then attribute 
        @ism:classification must have a value of [TS] or [S].
    </svrl:text>
            <svrl:text>
		If the document is an ISM_USGOV_RESOURCE, for each element which has 
		attribute @ism:atomicEnergyMarkings specified with a value containing 
		the token [RD-CNWDI], this rule ensures that the attribute @ism:classification
		has a value of [TS] or [S].
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M340"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00179</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00179</xsl:attribute>
            <svrl:text>
		Abstract pattern used to warn that an attribute value has a deprecation
		date in its CVE but has not passed based on the ISM_RESOURCE_CREATE_DATE of the resource.
		This pattern uses the deprecation dates in the CVE passed from the calling rule and the
		ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute has a depreciation date,
		which is a warning. The context, CVE name, and Spec name are passed from the calling
		rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M342"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00180</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00180</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that an attribute does not contain a deprecated token. 
		This pattern uses the deprecation dates in the CVE passed from the calling rule and 
		the ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute is deprecated, 
		which is an error. The context, CVE name, and Spec name are passed from the calling rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M343"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00181</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00181</xsl:attribute>
            <svrl:text>
        [ISM-ID-00181][Error] If ISM_USGOV_RESOURCE and element's classification does not have a value of "U" 
        then attribute @ism:atomicEnergyMarkings must not contain the name token [UCNI] or [DCNI].
        
        Human Readable: UCNI and DCNI may only be used on UNCLASSIFIED portions.
    </svrl:text>
            <svrl:text>
		If the document is an ISM_USGOV_RESOURCE, for each element which has 
		attribute @ism:atomicEnergyMarkings specified and has attribute @ism:classification specified with a value other than [U], 
		this rule ensures that attribute @ism:atomicEnergyMarkings does not contain the token [UCNI] or [DNCI].
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M344"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00183</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00183</xsl:attribute>
            <svrl:text>
        [ISM-ID-00183][Error] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings 
        contains a name token starting with [RD-SG], then it must also contain the name token [RD].
    </svrl:text>
            <svrl:text>
		If the document is an ISM_USGOV_RESOURCE, for each element which has 
		attribute @ism:atomicEnergyMarkings specified with a value containing a 
		token starting with [RD-SG], this rule ensures that attribute 
		@ism:atomicEnergyMarkings also contains the token [RD].
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M345"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00184</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00184</xsl:attribute>
            <svrl:text>
        [ISM-ID-00184][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:atomicEnergyMarkings contains a name token starting with [FRD-SG],
        then it must also contain the name token [FRD].
    </svrl:text>
            <svrl:text>
		If the document is an ISM_USGOV_RESOURCE, for each element which has 
		attribute @ism:atomicEnergyMarkings specified with a value containing a 
		token starting with [FRD-SG], this rule ensures that attribute 
		@ism:atomicEnergyMarkings also contains the token [FRD].
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M346"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00185</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00185</xsl:attribute>
            <svrl:text>
        [ISM-ID-00185][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:atomicEnergyMarkings contains the name token [RD-CNWDI],
        then it must also contain the name token [RD].
    </svrl:text>
            <svrl:text>
		If the document is an ISM_USGOV_RESOURCE, for each element which has 
		attribute @ism:atomicEnergyMarkings specified with a value containing 
		the token [RD-CNWDI], this rule ensures that attribute 
		@ism:atomicEnergyMarkings also contains the token [RD].
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M347"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00188</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00188</xsl:attribute>
            <svrl:text>
		Abstract pattern used to warn that an attribute value has a deprecation
		date in its CVE but has not passed based on the ISM_RESOURCE_CREATE_DATE of the resource.
		This pattern uses the deprecation dates in the CVE passed from the calling rule and the
		ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute has a depreciation date,
		which is a warning. The context, CVE name, and Spec name are passed from the calling
		rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M348"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00189</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00189</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that an attribute does not contain a deprecated token. 
		This pattern uses the deprecation dates in the CVE passed from the calling rule and 
		the ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute is deprecated, 
		which is an error. The context, CVE name, and Spec name are passed from the calling rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M349"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00190</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00190</xsl:attribute>
            <svrl:text>
		Abstract pattern used to warn that an attribute value has a deprecation
		date in its CVE but has not passed based on the ISM_RESOURCE_CREATE_DATE of the resource.
		This pattern uses the deprecation dates in the CVE passed from the calling rule and the
		ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute has a depreciation date,
		which is a warning. The context, CVE name, and Spec name are passed from the calling
		rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M350"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00191</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00191</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that an attribute does not contain a deprecated token. 
		This pattern uses the deprecation dates in the CVE passed from the calling rule and 
		the ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute is deprecated, 
		which is an error. The context, CVE name, and Spec name are passed from the calling rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M351"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00192</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00192</xsl:attribute>
            <svrl:text>
		Abstract pattern used to warn that an attribute value has a deprecation
		date in its CVE but has not passed based on the ISM_RESOURCE_CREATE_DATE of the resource.
		This pattern uses the deprecation dates in the CVE passed from the calling rule and the
		ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute has a depreciation date,
		which is a warning. The context, CVE name, and Spec name are passed from the calling
		rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M352"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00193</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00193</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that an attribute does not contain a deprecated token. 
		This pattern uses the deprecation dates in the CVE passed from the calling rule and 
		the ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute is deprecated, 
		which is an error. The context, CVE name, and Spec name are passed from the calling rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M353"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00196</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00196</xsl:attribute>
            <svrl:text>
		Abstract pattern used to warn that an attribute value has a deprecation
		date in its CVE but has not passed based on the ISM_RESOURCE_CREATE_DATE of the resource.
		This pattern uses the deprecation dates in the CVE passed from the calling rule and the
		ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute has a depreciation date,
		which is a warning. The context, CVE name, and Spec name are passed from the calling
		rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M354"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00197</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00197</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that an attribute does not contain a deprecated token. 
		This pattern uses the deprecation dates in the CVE passed from the calling rule and 
		the ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute is deprecated, 
		which is an error. The context, CVE name, and Spec name are passed from the calling rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M355"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00198</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00198</xsl:attribute>
            <svrl:text>
		Abstract pattern used to warn that an attribute value has a deprecation
		date in its CVE but has not passed based on the ISM_RESOURCE_CREATE_DATE of the resource.
		This pattern uses the deprecation dates in the CVE passed from the calling rule and the
		ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute has a depreciation date,
		which is a warning. The context, CVE name, and Spec name are passed from the calling
		rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M356"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00199</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00199</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that an attribute does not contain a deprecated token. 
		This pattern uses the deprecation dates in the CVE passed from the calling rule and 
		the ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute is deprecated, 
		which is an error. The context, CVE name, and Spec name are passed from the calling rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M357"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00200</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00200</xsl:attribute>
            <svrl:text>
		Abstract pattern used to warn that an attribute value has a deprecation
		date in its CVE but has not passed based on the ISM_RESOURCE_CREATE_DATE of the resource.
		This pattern uses the deprecation dates in the CVE passed from the calling rule and the
		ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute has a depreciation date,
		which is a warning. The context, CVE name, and Spec name are passed from the calling
		rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M358"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00201</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00201</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that an attribute does not contain a deprecated token. 
		This pattern uses the deprecation dates in the CVE passed from the calling rule and 
		the ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute is deprecated, 
		which is an error. The context, CVE name, and Spec name are passed from the calling rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M359"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00202</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00202</xsl:attribute>
            <svrl:text>
		Abstract pattern used to warn that an attribute value has a deprecation
		date in its CVE but has not passed based on the ISM_RESOURCE_CREATE_DATE of the resource.
		This pattern uses the deprecation dates in the CVE passed from the calling rule and the
		ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute has a depreciation date,
		which is a warning. The context, CVE name, and Spec name are passed from the calling
		rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M360"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00203</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00203</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that an attribute does not contain a deprecated token. 
		This pattern uses the deprecation dates in the CVE passed from the calling rule and 
		the ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute is deprecated, 
		which is an error. The context, CVE name, and Spec name are passed from the calling rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M361"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00204</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00204</xsl:attribute>
            <svrl:text>
		Abstract pattern used to warn that an attribute value has a deprecation
		date in its CVE but has not passed based on the ISM_RESOURCE_CREATE_DATE of the resource.
		This pattern uses the deprecation dates in the CVE passed from the calling rule and the
		ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute has a depreciation date,
		which is a warning. The context, CVE name, and Spec name are passed from the calling
		rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M362"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00205</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00205</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that an attribute does not contain a deprecated token. 
		This pattern uses the deprecation dates in the CVE passed from the calling rule and 
		the ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute is deprecated, 
		which is an error. The context, CVE name, and Spec name are passed from the calling rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M363"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00206</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00206</xsl:attribute>
            <svrl:text>
		Abstract pattern used to warn that an attribute value has a deprecation
		date in its CVE but has not passed based on the ISM_RESOURCE_CREATE_DATE of the resource.
		This pattern uses the deprecation dates in the CVE passed from the calling rule and the
		ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute has a depreciation date,
		which is a warning. The context, CVE name, and Spec name are passed from the calling
		rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M364"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00207</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00207</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that an attribute does not contain a deprecated token. 
		This pattern uses the deprecation dates in the CVE passed from the calling rule and 
		the ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute is deprecated, 
		which is an error. The context, CVE name, and Spec name are passed from the calling rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M365"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00208</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00208</xsl:attribute>
            <svrl:text>
		Abstract pattern used to warn that an attribute value has a deprecation
		date in its CVE but has not passed based on the ISM_RESOURCE_CREATE_DATE of the resource.
		This pattern uses the deprecation dates in the CVE passed from the calling rule and the
		ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute has a depreciation date,
		which is a warning. The context, CVE name, and Spec name are passed from the calling
		rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M366"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00209</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00209</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that an attribute does not contain a deprecated token. 
		This pattern uses the deprecation dates in the CVE passed from the calling rule and 
		the ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute is deprecated, 
		which is an error. The context, CVE name, and Spec name are passed from the calling rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M367"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00210</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00210</xsl:attribute>
            <svrl:text>
		Abstract pattern used to warn that an attribute value has a deprecation
		date in its CVE but has not passed based on the ISM_RESOURCE_CREATE_DATE of the resource.
		This pattern uses the deprecation dates in the CVE passed from the calling rule and the
		ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute has a depreciation date,
		which is a warning. The context, CVE name, and Spec name are passed from the calling
		rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M368"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00211</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00211</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that an attribute does not contain a deprecated token. 
		This pattern uses the deprecation dates in the CVE passed from the calling rule and 
		the ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute is deprecated, 
		which is an error. The context, CVE name, and Spec name are passed from the calling rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M369"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00213</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00213</xsl:attribute>
            <svrl:text>
        [ISM-ID-00213][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:disseminationControls contains the name token [DISPLAYONLY], then 
        attribute @ism:displayOnlyTo must be specified.
        
        Human Readable: A USA document with DISPLAY ONLY dissemination must 
        indicate the countries to which it may be disclosed.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [DISPLAYONLY] this rule ensures that attribute @ism:displayOnlyTo
    	is specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M370"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00214</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00214</xsl:attribute>
            <svrl:text>
        [ISM-ID-00214][Error] If ISM_USGOV_RESOURCE then attribute @ism:releasableTo must start with [USA].
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:releasableTo this rule ensures that attribute
        @ism:releasableTo is specified with a value that starts with the token [USA].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M371"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00217</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00217</xsl:attribute>
            <svrl:text>
        [ISM-ID-00217][Error] If ISM_USGOV_RESOURCE attribute @ism:FGIsourceProtected contains [FGI], it must be the only value.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE, for each element which specifies
    	the attribute @ism:FGIsourceProtected, this rule ensures that attribute
    	@ism:FGIsourceProtected contains only the token [FGI].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M372"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00221</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00221</xsl:attribute>
            <svrl:text>
        [ISM-ID-00221][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:derivativelyClassifiedBy is specified, then attributes @ism:classificationReason
        or @ism:classifiedBy must not be specified.
        
        Human Readable: USA documents that are derivatively classified must not
        specify a classification reason or classified by.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE, for each element which 
    	specifies attribute @ism:derivativelyClassifiedBy this rule ensures that
    	attribute @ism:classificationReason or @ism:classifiedBy is NOT specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M374"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00223</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00223</xsl:attribute>
            <svrl:text>This abstract pattern checks to see if an attribute of an element exists
        in a list. The calling rule must pass ism:*, local-name(), $validElementList, '   [ISM-ID-00223][Error] If any elements in namespace    urn:us:gov:ic:ism exist, the local name must exist in CVEnumISMElements.xml.       Human Readable: Ensure that elements in the ISM namespace are defined by ISM.XML.   '.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M375"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00226</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00226</xsl:attribute>
            <svrl:text>
        [ISM-ID-00226][Error] Attributes @ism:noticeType and @ism:unregisteredNoticeType
        may not both be used on the same element. 
        
        Human Readable: Ensure that the ISM attributes noticeType and
        unregisteredNoticeType are not used on the same element.
    </svrl:text>
            <svrl:text>
        For each element which has attribute ism:noticeType specified, this rule ensures that ism:unregisteredNoticeType
        is not specified. 
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M376"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00242</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00242</xsl:attribute>
            <svrl:text>
        [ISM-ID-00242][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [RSV],
        then it must also have attribute @ism:classification with a value of [S] or [TS].
        
        Human Readable: A USA document that contains RESERVE data must be classified SECRET or TOP SECRET.
    </svrl:text>
            <svrl:text>
      If the document is an ISM_USGOV_RESOURCE, for each element which specifies attribute @ism:SCIcontrols 
      with a value containing the token [RSV], this rule ensures that attribute ism:classification is 
      specified with a value containing the token [TS] or [S].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M381"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00243</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00243</xsl:attribute>
            <svrl:text>
    [ISM-ID-00243][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [RSV],
    then it must also contain a compartment [RSV-XXX].
    
    Human Readable: RESERVE is not permitted as a stand-alone value and a compartment must be expressed.
  </svrl:text>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE, for each element which specifies attribute @ism:SCIcontrols 
    with a value containing the token [RSV], this rule ensures that attribute @ism:SCIcontrols is 
    specified with a value containing a token maching the regular expression "RSV-[A-Z0-9]{3}".
    
    If IC Markings System Register and Manual rules do not apply to the document then the rule does not apply
    and this rule returns true. If the current element has attribute @ism:SCIcontrols specified
    with a value containing [RSV], then this rule ensures that attribute @ism:SCIcontrols also contains the value [RSV-XXX].
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M382"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00244</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00244</xsl:attribute>
            <svrl:text>
    [ISM-ID-00244][Error] If ISM_USGOV_RESOURCE and:
    1. Any element meeting ISM_CONTRIBUTES in the document has the attribute @ism:atomicEnergyMarkings containing [RD-CNWDI]
    AND
    2. No element meeting ISM_CONTRIBUTES in the document has @ism:noticeType containing [CNWDI].
    that does not have attribute @ism:externalNotice with a value of [true].
    
    Human Readable: USA documents containing CNWDI data must also have an CNWDI notice.
  </svrl:text>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE, for each element meeting
    ISM_CONTRIBUTES which specifies attribute @ism:atomicEnergyMarkings with
    a value containing the token [RD-CNWDI], then this rule ensures that some element
    in the document specifies attribute @ism:noticeType with a value containing
    the token [CNWDI] and not an attribute @ism:externalNotice with a value of [true].
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M383"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00245</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00245</xsl:attribute>
            <svrl:text>
        [ISM-ID-00245][Error] If ISM_USGOV_RESOURCE and:
        1. No element without @ism:excludeFromRollup=true() in the document has the attribute @ism:atomicEnergyMarkings containing [RD-CNWDI]
        AND
        2. Any element without @ism:excludeFromRollup=true() in the document has the attribute @ism:noticeType containing [CNWDI]
        and not the attribute @ism:externalNotice with a value of [true].
        
        Human Readable: USA documents containing an CNWDI notice must also have RD-CNWDI data.
    </svrl:text>
            <svrl:text>
      If the document is an ISM_USGOV_RESOURCE, for each element which meets
      ISM_CONTRIBUTES and specifies attribute @ism:noticeType with a value
      containing the token [CNWDI] and not the attribute @ism:externalNotice with a value of [true], 
      then this rule ensures that some element in the document specifies attribute @ism:atomicEnergyMarkings with a value
      containing the token [RD-CNWDI].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M384"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00250</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00250</xsl:attribute>
            <svrl:text>
		[ISM-ID-00250][Error] If ISM_USGOV_RESOURCE, element ism:Notice must specify 
		attribute @ism:noticeType or @ism:unregisteredNoticeType.
		
		Human Readable: Notices must specify their type.
	</svrl:text>
            <svrl:text>
		This rule ensures for element ism:Notice must specify their type.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M386"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00253</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00253</xsl:attribute>
            <svrl:text>
        This abstract pattern checks to see if an attribute of an element exists
        in a list or matches the pattern defined by the list. The calling rule must pass the
        context, search term list, attribute value to check, and an error message.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M388"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00254</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00254</xsl:attribute>
            <svrl:text>
        This abstract pattern checks to see if an attribute of an element exists
        in a list or matches the pattern defined by the list. The calling rule must pass the
        context, search term list, attribute value to check, and an error message.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M389"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00255</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00255</xsl:attribute>
            <svrl:text>
        This abstract pattern checks to see if an attribute of an element exists
        in a list or matches the pattern defined by the list. The calling rule must pass the
        context, search term list, attribute value to check, and an error message.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M390"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00256</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00256</xsl:attribute>
            <svrl:text>
        This abstract pattern checks to see if an attribute of an element exists
        in a list or matches the pattern defined by the list. The calling rule must pass the
        context, search term list, attribute value to check, and an error message.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M391"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00257</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00257</xsl:attribute>
            <svrl:text>
        This abstract pattern checks to see if an attribute of an element exists
        in a list or matches the pattern defined by the list. The calling rule must pass the
        context, search term list, attribute value to check, and an error message.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M392"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00258</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00258</xsl:attribute>
            <svrl:text>
        This abstract pattern checks to see if an attribute of an element exists
        in a list or matches the pattern defined by the list. The calling rule must pass the
        context, search term list, attribute value to check, and an error message.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M393"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00259</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00259</xsl:attribute>
            <svrl:text>
        This abstract pattern checks to see if an attribute of an element exists
        in a list or matches the pattern defined by the list. The calling rule must pass the
        context, search term list, attribute value to check, and an error message.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M394"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00260</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00260</xsl:attribute>
            <svrl:text>
        This abstract pattern checks to see if an attribute of an element exists
        in a list or matches the pattern defined by the list. The calling rule must pass the
        context, search term list, attribute value to check, and an error message.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M395"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00261</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00261</xsl:attribute>
            <svrl:text>
        This abstract pattern checks to see if the attribute values of an element 
        exists in a list or matches the pattern defined by the list when these values are flagged as 
        contributing to rollup. The calling rule must pass the context, search term list, attribute value 
        to check, flag on whether the attribute values contribute to rollup, and an error message.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M396"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00262</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00262</xsl:attribute>
            <svrl:text>
        This abstract pattern checks to see if an attribute of an element exists
        in a list or matches the pattern defined by the list. The calling rule must pass the
        context, search term list, attribute value to check, and an error message.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M397"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00263</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00263</xsl:attribute>
            <svrl:text>
        This abstract pattern checks to see if an attribute of an element exists
        in a list or matches the pattern defined by the list. The calling rule must pass the
        context, search term list, attribute value to check, and an error message.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M398"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00264</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00264</xsl:attribute>
            <svrl:text>
        This abstract pattern checks to see if an attribute of an element exists
        in a list or matches the pattern defined by the list. The calling rule must pass the
        context, search term list, attribute value to check, and an error message.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M399"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00265</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00265</xsl:attribute>
            <svrl:text>
        This abstract pattern checks to see if an attribute of an element exists
        in a list or matches the pattern defined by the list. The calling rule must pass the
        context, search term list, attribute value to check, and an error message.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M400"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00266</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00266</xsl:attribute>
            <svrl:text>
        This abstract pattern checks to see if the attribute values of an element 
        exists in a list or matches the pattern defined by the list when these values are flagged as 
        contributing to rollup. The calling rule must pass the context, search term list, attribute value 
        to check, flag on whether the attribute values contribute to rollup, and an error message.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M401"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00267</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00267</xsl:attribute>
            <svrl:text>
        This abstract pattern checks to see if the attribute values of an element 
        exists in a list or matches the pattern defined by the list when these values are flagged as 
        contributing to rollup. The calling rule must pass the context, search term list, attribute value 
        to check, flag on whether the attribute values contribute to rollup, and an error message.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M402"/>
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
            <xsl:attribute name="id">ISM-ID-00299</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00299</xsl:attribute>
            <svrl:text>
        [ISM-ID-00299][Error] If an element contains the attribute @ism:declassException with a value of [AEA], 
        it must also contain the attribute @ism:atomicEnergyMarkings.
    </svrl:text>
            <svrl:text>
		If an element contains an @ism:declassException attribute with a value containing
		[AEA], this rule checks to make sure that element also has an @ism:atomicEnergyMarkings
		attribute.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M434"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00302</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00302</xsl:attribute>
            <svrl:text>
        [ISM-ID-00302][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:disseminationControls contains the name token [OC-USGOV], then 
        name token [OC] must be specified.
        
        Human Readable: A USA document with OC-USGOV dissemination must 
        also contain an OC dissemination.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [OC-USGOV], this rule ensures that token [OC] is also specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M435"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00313</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00313</xsl:attribute>
            <svrl:text>
        [ISM-ID-00313][Error] If @ism:nonICmarkings contains the token [ND] then the 
        attribute @ism:disseminationControls must contain [NF].
        
        Human Readable: NODIS data must be marked NOFORN.
    </svrl:text>
            <svrl:text>
        If the @ism:nonICmarkings contains the ND token, then check that the @ism:disseminationControls
        attribute must have NF specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M437"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00314</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00314</xsl:attribute>
            <svrl:text>
        [ISM-ID-00314][Error] If @ism:nonICmarkings contains the token [XD] then the 
        attribute @ism:disseminationControls must contain [NF].
        
        Human Readable: EXDIS data must be marked NOFORN.
    </svrl:text>
            <svrl:text>
        If the @ism:nonICmarkings contains the ND token, then check that the @ism:disseminationControls
        attribute must have NF specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M438"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00319</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00319</xsl:attribute>
            <svrl:text>
        [ISM-ID-00319][Error] If ISM_USGOV_RESOURCE and @ism:ownerProducer contains 'USA' and attribute
        @ism:releasableTo is specified, then @ism:releasableTo must contain more than a single token.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE and a portion's @ism:ownerProducer attribute contains 'USA' and specifies
        attribute @ism:releasableTo, this rule ensures that the token count for releasableTo is greater than
        1.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M443"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00321</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00321</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that mutually exclusive tokens do not exist in
		an attribute. The calling rule must pass @ism:atomicEnergyMarkings and ('RD', 'FRD', 'TFNI').</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M445"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00325</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00325</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that mutually exclusive tokens do not exist in
		an attribute. The calling rule must pass @ism:disseminationControls and ('OC', 'RELIDO').</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M447"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00327</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00327</xsl:attribute>
            <svrl:text>
        [ISM-ID-00327][Error] If ISM_USGOV_RESOURCE and: 
        1. Any element in the document that has the attribute @ism:disseminationControls containing [FOUO]
        AND
        2. Has the attribute @ism:classification [U]
        Then the element can only have the @ism:disseminationControls containing [REL], [RELIDO], [NF], [DISPLAYONLY], and [EYES].
        
        Human Readable: Dissemination control markings, excluding Foreign Disclosure and Release markings 
        (REL, RELIDO, NF, DISPLAYONLY, or EYES), in elements of USA Unclassified documents supersede and take precedence 
        over FOUO.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for any element that contains @ism:disseminationControls
        with a value containing [FOUO] and has @ism:classification with a value of [U], 
        then this rule ensures that @ism:disseminationControls only contains the
        tokens [REL], [RELIDO], [NF], [EYES], [DISPLAYONLY], or [FOUO].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M449"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00328</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00328</xsl:attribute>
            <svrl:text>
        [ISM-ID-00328][Error] If ISM_USGOV_RESOURCE and: 
        1. Any element in the document that has the attribute @ism:disseminationControls containing [FOUO]
        AND
        2. Has the attribute @ism:classification [U]
        
        Then the element can't have any @ism:nonICMarkings.
        
        Human Readable: Non-IC dissemination control markings in elements of USA Unclassified documents 
        supersede and take precedence over FOUO.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for any element that contains @ism:disseminationControls
        with a value containing [FOUO] and has @ism:classification with a value of [U], 
        then this rule ensures that there is no @ism:nonICMarkings.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M450"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00330</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00330</xsl:attribute>
            <svrl:text>
        [ISM-ID-00330][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [HCS-P], then attribute 
        @ism:classification must have a value of [TS], or [S].
        
        Human Readable: A USA document with HCS-PRODUCT compartment data must be classified SECRET or TOP SECRET.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [HCS-P] ensure that attribute @ism:classification is specified with a value containing the token [TS], or [S].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M451"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00332</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00332</xsl:attribute>
            <svrl:text>
        [ISM-ID-00332][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [HCS-O], 
        then attribute @ism:classification must have a value of [TS] or [S].
        
        Human Readable: A USA document with HCS-OPERATIONS compartment data must be classified SECRET or TOP SECRET.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [HCS-O], ensure that attribute @ism:classification is specified with a value of [TS] or [S].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M452"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00335</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00335</xsl:attribute>
            <svrl:text>
        [ISM-ID-00335][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [HCS-O],
        then attribute @ism:disseminationControls must contain the name token [OC].
        
        Human Readable: A USA document with HCS-OPERATIONS compartment data must be marked for 
        ORIGINATOR CONTROLLED dissemination.
    </svrl:text>
            <svrl:text>
      If the document is an ISM_USGOV_RESOURCE, for each element which
      specifies attribute @ism:SCIcontrols with a value containing the token
      [HCS-O], this rule ensures that attribute @ism:disseminationControls is
      specified with a value containing the token [OC].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M453"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00336</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00336</xsl:attribute>
            <svrl:text>
        [ISM-ID-00336][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains a token matching [HCS-P-XXXXXX], 
        where X is represented by the regular expression character class [A-Z0-9]{1,6}, then attribute
        @ism:disseminationControls must contain the name token [OC].
        
        Human Readable: A USA document with HCS-PRODUCT subcompartment data must be marked for 
        ORIGINATOR CONTROLLED dissemination.
    </svrl:text>
            <svrl:text>
      If the document is an ISM_USGOV_RESOURCE, for each element which
      specifies attribute @ism:SCIcontrols with a value containing a token matching
      [HCS-P-XXXXXX], where X is represented by the regular expression character
      class [A-Z0-9]{1,6}, this rule ensures that attribute @ism:disseminationControls is
      specified with a value containing the token [OC].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M454"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00341</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00341</xsl:attribute>
            <svrl:text>
        [ISM-ID-00341][Error] If ISM_USGOV_RESOURCE and @ism:SCIcontrols contains a token matching [SI-G]
        or [SI-G-XXXX], then @ism:disseminationControls cannot contain [OC-USGOV].
        
        Human Readable: OC-USGOV cannot be used if SI-G or an SI-G subs are present. 
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE and @ism:SCIcontrols contains [SI-G] or [SI-G-XXXX], 
        then @ism:disseminationControls cannot contain [OC-USGOV].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M455"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00345</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00345</xsl:attribute>
            <svrl:text>
	  	[ISM-ID-00345][Error] If ISM_USGOV_RESOURCE and attribute @ism:disseminationControls contains the value [EYES], 
	  	@ism:releasableTo must only contain the token values of [USA], [AUS], [CAN], [GBR] or [NZL]. 
	  </svrl:text>
            <svrl:text>
	  	If ISM_USGOV_RESOURCE, for each element which specifies the attribute @ism:disseminationControls with the value of [EYES], 
	  	this rule ensures that attribute @ism:releasableTo is specified with the token values of [USA], [AUS], [CAN], [GBR] or [NZL].	  
	  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M458"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00346</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00346</xsl:attribute>
            <svrl:text>
	  	[ISM-ID-00346][Error] If ISM_USGOV_RESOURCE and attribute @ism:nonICmarkings contains the name token [DS], 
	  	then attribute @ism:classification must have a value of [U].
	  	
	  	Human Readable: The DS (LIMDIS) nonICmarkings value in a USA document
	  	must only be used with a classification of UNCLASSIFIED.
	</svrl:text>
            <svrl:text>
	  	If the document is an ISM_USGOV_RESOURCE, for each element which has 
	  	attribute @ism:nonICmarkings specified with a value containing
	  	the token [DS] this rule ensures that attribute @ism:classification is 
	  	specified with a value of [U].
	  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M459"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00352</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00352</xsl:attribute>
            <svrl:text>
      Abstract template to validate that for an $ISM_USGOV_RESOURCE or
      $ISM_USCUIONLY_RESOURCE, one of two given tokens ('PR' or 'PROPIN') exists in a particular attribute of at
      least one of 
      (a) a portion that contributes to roll-up or 
      (b) the banner, given the existence
      of an ntk:AccessProfile that has an ntk:AccessPolicy value that starts with a given string
      ('urn:us:gov:ic:aces:ntk:propin:').
   </svrl:text>
            <svrl:text>
      Expected parameters: 'ISM-ID-00352', 'PROPIN', 'urn:us:gov:ic:aces:ntk:propin:', 'disseminationControls or cuiBasic or cuiSpecified', 'PR', 'PROPIN',
      ($partDisseminationControls_tok,$partCuiBasic_tok,$partCuiSpecified_tok), and ($bannerDisseminationControls_tok,$bannerCuiBasic_tok,$bannerCuiSpecified_tok)
   </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M465"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00353</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00353</xsl:attribute>
            <svrl:text>
      Abstract template to validate that for an $ISM_USGOV_RESOURCE, a given token ('OC')
      exists in a particular attribute of at least one of 
      (a) a portion that contributes to roll-up or 
      (b) the banner, given the existence of an ntk:AccessProfile that has an ntk:AccessPolicy value that starts with a given string
      ('urn:us:gov:ic:aces:ntk:oc').
   </svrl:text>
            <svrl:text>
      Expected parameters: 'ISM-ID-00353', 'ORCON', 'urn:us:gov:ic:aces:ntk:oc', 'disseminationControls', 'OC', $partDisseminationControls_tok, and
      $bannerDisseminationControls_tok
   </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M466"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00354</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00354</xsl:attribute>
            <svrl:text>
      Abstract template to validate that for an $ISM_USGOV_RESOURCE, a given token ('XD')
      exists in a particular attribute of at least one of 
      (a) a portion that contributes to roll-up or 
      (b) the banner, given the existence of an ntk:AccessProfile that has an ntk:AccessPolicy value that starts with a given string
      ('urn:us:gov:ic:aces:ntk:xd').
   </svrl:text>
            <svrl:text>
      Expected parameters: 'ISM-ID-00354', 'EXDIS', 'urn:us:gov:ic:aces:ntk:xd', 'nonICmarkings', 'XD', $partNonICmarkings_tok, and
      $bannerNonICmarkings_tok
   </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M467"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00355</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00355</xsl:attribute>
            <svrl:text>
      Abstract template to validate that for an $ISM_USGOV_RESOURCE, a given token ('ND')
      exists in a particular attribute of at least one of 
      (a) a portion that contributes to roll-up or 
      (b) the banner, given the existence of an ntk:AccessProfile that has an ntk:AccessPolicy value that starts with a given string
      ('urn:us:gov:ic:aces:ntk:nd').
   </svrl:text>
            <svrl:text>
      Expected parameters: 'ISM-ID-00355', 'NODIS', 'urn:us:gov:ic:aces:ntk:nd', 'nonICmarkings', 'ND', $partNonICmarkings_tok, and
      $bannerNonICmarkings_tok
   </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M468"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00356</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00356</xsl:attribute>
            <svrl:text>
		Abstract pattern to enforce that an appropriate notice exists for an
		element in $partTags that has a notice requirement. The calling rule must pass $elem,
		'ism:nonICmarkings', $partTags, and 'SSI'.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M469"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00357</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00357</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that for a given element in a document that is
		ISM_USGOV_RESOURCE with no CUI, with @ism:noticeType containing a specified token and @ism:externalNotice
		not equal true, 'SSI' exists in $partNonICmarkings_tok. The calling rule must pass 'SSI',
		'SSI' and @dataTokenList.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M470"/>
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
            <xsl:attribute name="id">ISM-ID-00362</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00362</xsl:attribute>
            <svrl:text>
        [ISM-ID-00362][Error] HCS-P-subs cannot be used with OC-USGOV.
    </svrl:text>
            <svrl:text>
        When OC-USGOV @ism:disseminationControls is used, tokens matching the regular expression 
        HCS-P-[A-Z0-9]{1,6} cannot be in @ism:SCIcontrols.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M472"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00363</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00363</xsl:attribute>
            <svrl:text>
        [ISM-ID-00363][Error] HCS-O cannot be used with OC-USGOV.
    </svrl:text>
            <svrl:text>
        When OC-USGOV @ism:disseminationControls is used, HCS-O cannot be in @ism:SCIcontrols.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M473"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00368</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00368</xsl:attribute>
            <svrl:text>
        [ISM-ID-00368][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains the name token [TK-BLFH], then attribute @ism:classification must have
        a value of [TS].
        
        Human Readable: A USA document containing TALENT KEYHOLE (TK) -BLUEFISH compartment data must
        be classified TOP SECRET.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [TK-BLFH] this rule ensures that attribute @ism:classification is 
        specified with a value containing the token [TS].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M477"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00369</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00369</xsl:attribute>
            <svrl:text>
        [ISM-ID-00369][Error] If ISM_USGOV_RESOURCE and @ism:attribute SCIcontrols
        contains the name token [TK-BLFH], then attribute @ism:disseminationControls
        must contain the name token [NF].
        
        Human Readable: A USA document containing TALENT KEYHOLE (TK) -BLUEFISH compartment data must also be
        marked for NO FOREIGN dissemination.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [TK-BLFH] this rule ensures that attribute @ism:disseminationControls is 
        specified with a value containing the token [NF].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M478"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00370</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00370</xsl:attribute>
            <svrl:text>
        [ISM-ID-00370][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains the name token [TK-IDIT], then attribute @ism:disseminationControls
        must contain the name token [NF].
        
        Human Readable: A USA document containing TALENT KEYHOLE (TK) -IDITAROD compartment data must also be
        marked for NO FOREIGN dissemination.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [TK-IDIT] this rule ensures that attribute @ism:disseminationControls is 
        specified with a value containing the token [NF].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M479"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00371</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00371</xsl:attribute>
            <svrl:text>
        [ISM-ID-00371][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains the name token [TK-KAND], then attribute @ism:disseminationControls
        must contain the name token [NF].
        
        Human Readable: A USA document containing TALENT KEYHOLE (TK) -KANDIK compartment data must also be
        marked for NO FOREIGN dissemination.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [TK-KAND] this rule ensures that attribute @ism:disseminationControls is 
        specified with a value containing the token [NF].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M480"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00372</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00372</xsl:attribute>
            <svrl:text>
        [ISM-ID-00372][Error] If ISM_USGOV_RESOURCE and attribute @ism:nonICmarkings
        contains the name token [LES-NF] or [SBU-NF], then attribute @ism:disseminationControls
        must not contain the name token [NF], [REL], [EYES], [RELIDO], or [DISPLAYONLY].
        
        Human Readable: LES-NF and SBU-NF are incompatible with other Foreign Disclosure 
        and Release markings.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:nonICmarkings with a value containing the token
        [LES-NF] or [SBU-NF] this rule ensures that attribute @ism:disseminationControls is 
        not specified with a value containing the token [NF], [REL], [EYES], [RELIDO], or 
        [DISPLAYONLY].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M481"/>
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
            <xsl:attribute name="id">ISM-ID-00384</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00384</xsl:attribute>
            <svrl:text>
		Abstract pattern to enforce that an appropriate notice exists for an
		element in $partTags that has a notice requirement. The calling rule must pass $elem,
		'disseminationControls', $partTags, and 'RSEN', 'IMCON_RSEN'.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M486"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00385</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00385</xsl:attribute>
            <svrl:text>
        [ISM-ID-00385][Error] Attribute @ism:declassEvent requires use of attribute @ism:declassDate. 
    </svrl:text>
            <svrl:text>
        CFR policies require that @ism:declassDate accompany @ism:declassEvent. Set context to any element 
        containing @ism:declassEvent attribute. Test if that element also has @ism:declassDate.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M487"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00386</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00386</xsl:attribute>
            <svrl:text>Abstract pattern to enforce that an appropriate notice exists for an
		element in $partTags that has a notice requirement. The calling rule must pass $elem,
		'SCIcontrols', $partTags, and 'GEOCAP'.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M488"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00387</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00387</xsl:attribute>
            <svrl:text>Abstract pattern to ensure that for a given element in an
		ISM_USGOV_RESOURCE with @ism:noticeType containing a specified token and ism:externalNotice
		not equal true, 'TK-.*' exists in $partSCIcontrols_tok. The calling rule must pass 'TK-.*' and
		@dataTokenList.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M489"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00388</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00388</xsl:attribute>
            <svrl:text>
    [ISM-ID-00388][Error] If ISM_USGOV_RESOURCE and @ism:attribute SCIcontrols contains a token matching containing a "-" 
    then it must also contain the token before the "-". This is to ensure all compartments specify the control system 
    and all subcompartments specify the compartment. 
    
    Human Readable: A USA document with a SCI compartment must specify the control system, 
    also a SCI subcompartment must specify the compartment. 
  </svrl:text>
            <svrl:text>
    If ISM_USGOV_RESOURCE and attribute SCIcontrols contains a token matching containing a "-" 
    then it must also contain the token before the "-". This is to ensure all compartments specify the control system 
    and all subcompartments specify the compartment.
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M490"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00391</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00391</xsl:attribute>
            <svrl:text>
		Abstract pattern to enforce that an appropriate notice exists for an
		element in $partTags that has a notice requirement. The calling rule must pass $elem,
		'disseminationControls', $partTags, and 'RAWFISA'.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M492"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00392</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00392</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that for a given element in a document that is
		ISM_USGOV_RESOURCE with no CUI, with @ism:noticeType containing a specified token and @ism:externalNotice
		not equal true, 'RAWFISA' exists in $partDisseminationControls_tok. The calling rule must pass 'RAWFISA',
		'RAWFISA' and @dataTokenList.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M493"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00393</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00393</xsl:attribute>
            <svrl:text>
        [ISM-ID-00393][Error] If ISM_USGOV_RESOURCE and attribute @ism:disseminationControls
        contains the name token [RAWFISA], then attribute @ism:classification must have
        a value of [TS] or [S].
        
        Human Readable: A USA document containing RAWFISA data must be classified
        SECRET or TOP SECRET.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:disseminationControls with a value containing the token
        [RAWFISA] this rule ensures that attribute @ism:classification is 
        specified with a value containing the token [TS] or [S].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M494"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00396</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00396</xsl:attribute>
            <svrl:text>
        [ISM-ID-00396][Warning] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [KLM], 
        then [KLM] SHOULD contain [NF]; ensure you have proper release authority from the KLM program.
        
        Human Readable: A USA document containing KLM data is usually NOFORN; ensure you have proper release
        authority from the KLM program.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [KLM] this rule checks that attribute @ism:disseminationControls is 
        specified with a value containing the token [NF] and gives a WARNING if there is no [NF].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M496"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00397</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00397</xsl:attribute>
            <svrl:text>
        [ISM-ID-00397][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains a name token that complies with the pattern [KLM-] followed by any alphanumeric string, then attribute
        @ism:disseminationControls must contain the name token [OC], except for the [KLM-R] compartment which does not require [OC].
        
        Human Readable: A USA document containing a KLM compartment data must be marked for ORIGINATOR CONTROLLED (ORCON)
        dissemination, except for the KLM-R compartment which does not require ORCON dissemination.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing a token
        starting with [KLM-], this rule ensures that attribute
        @ism:disseminationControls is specified with a value containing the
        token [OC]. The one exception to the requirement for [OC] is the [KLM-R] compartment.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M497"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00398</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00398</xsl:attribute>
            <svrl:text>
        [ISM-ID-00398][Error] If ISM_USGOV_RESOURCE and @ism:attribute SCIcontrols
        contains a name token that complies with the pattern [KLM-X-Y], where X and Y are any alphanumeric
        strings of any length, then attribute @ism:disseminationControls must contain the name token [OC].
        
        Human Readable: A USA document with any KLM subcompartments must be marked for ORIGINATOR CONTROLLED (ORCON) dissemination.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing a token
        following the pattern [KLM-X-Y], where X and Y are any alphanumeric strings of any length, this rule ensures that attribute
        @ism:disseminationControls is specified with a value containing the token [OC].  
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M498"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00441</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00441</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that for a given element in a document that is
		ISM_USGOV_RESOURCE with no CUI, with @ism:noticeType containing a specified token and @ism:externalNotice
		not equal true, 'RS' exists in $partDisseminationControls_tok. The calling rule must pass 'RS',
		'RSEN' and @dataTokenList.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M499"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00442</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00442</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that for a given element in a document that is
		ISM_USGOV_RESOURCE with no CUI, with @ism:noticeType containing a specified token and @ism:externalNotice
		not equal true, 'RS' exists in $partDisseminationControls_tok. The calling rule must pass 'RS',
		'IMCON_RSEN' and @dataTokenList.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M500"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00443</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00443</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that for a given element in a document that is
		ISM_USGOV_RESOURCE with no CUI, with @ism:noticeType containing a specified token and @ism:externalNotice
		not equal true, 'IMC' exists in $partDisseminationControls_tok. The calling rule must pass 'IMC',
		'IMCON_RSEN' and @dataTokenList.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M501"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00444</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00444</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that for a given element in a document that is
		ISM_USGOV_RESOURCE with no CUI, with @ism:noticeType containing a specified token and @ism:externalNotice
		not equal true, 'IMC' exists in $partDisseminationControls_tok. The calling rule must pass 'IMC',
		'IMC' and @dataTokenList.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M502"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00459</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00459</xsl:attribute>
            <svrl:text>
        [ISM-ID-00459][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [HCS-X], 
        then attribute @ism:classification must have a value of [TS] or [S].
        
        Human Readable: A USA document with HCS-EXTERNAL compartment data must be classified SECRET or TOP SECRET.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [HCS-X], ensure that attribute @ism:classification is specified with a value of [TS] or [S].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M503"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00462</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00462</xsl:attribute>
            <svrl:text>
        [ISM-ID-00462][Error] If ISM_USGOV_RESOURCE and attribute @ism:classification is [U], then attribute @ism:nonICmarkings
        must not contain a name token that starts with ACCM.
        
        Human Readable: A USA document containing ACCM data must be classified CONFIDENTIAL, SECRET, or TOP SECRET.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which specifies attribute @ism:classification='U', 
        then this rule ensures that @ism:nonICmarkings does not contain a token that starts with ACCM.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M506"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00463</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00463</xsl:attribute>
            <svrl:text>
        [ISM-ID-00463][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains the name token [BUR], then attribute @ism:disseminationControls
        must contain the name token [NF].
        
        Human Readable: A USA document containing BUR data must be marked
        for NO FOREIGN dissemination.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [BUR] this rule ensures that attribute @ism:disseminationControls is 
        specified with a value containing the token [NF].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M507"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00464</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00464</xsl:attribute>
            <svrl:text>
        [ISM-ID-00464][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains the name token [RSV], then attribute @ism:disseminationControls
        must contain the name token [NF].
        
        Human Readable: A USA document containing RSV data must be marked
        for NO FOREIGN dissemination.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [RSV] this rule ensures that attribute @ism:disseminationControls is 
        specified with a value containing the token [NF].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M508"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00465</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00465</xsl:attribute>
            <svrl:text>
        [ISM-ID-00465][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains the name token [BUR], then attribute @ism:classification must have
        a value of [TS] or [S].
        
        Human Readable: A USA document containing BUR data must be classified
        SECRET or TOP SECRET.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [BUR] this rule ensures that attribute @ism:classification is 
        specified with a value containing the token [TS] or [S].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M509"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00466</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00466</xsl:attribute>
            <svrl:text>
        [ISM-ID-00466][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains the name token [KLM], then attribute @ism:classification must have
        a value of [TS] or [S].
        
        Human Readable: A USA document containing KLM data must be classified
        SECRET or TOP SECRET.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [KLM] this rule ensures that attribute @ism:classification is 
        specified with a value containing the token [TS] or [S].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M510"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00467</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00467</xsl:attribute>
            <svrl:text>
        [ISM-ID-00467][Warning] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings
        contains one of the name tokens [RD] or [FRD], then [RD] and [FRD] SHOULD contain [NF].
        In order to release [RD] or [FRD] data to a foreign partner, ensure you have established a sharing
        agreement per the AEA. 
        
        Human Readable: A USA document containing RD and/or FRD data is usually NOFORN;
        ensure you have proper release authority per the AEA. 
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which specifies
        attribute @ism:atomicEnergyMarkings with a value containing one of the tokens [RD] or [FRD], this rule checks
        that attribute @ism:disseminationControls is specified with a value containing the token [NF]
        and gives a WARNING if there is no [NF].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M511"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00468</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00468</xsl:attribute>
            <svrl:text>
        [ISM-ID-00468][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains a token starting with [KLM-R], then attribute @ism:classification must have
        a value of [TS].
        
        Human Readable: A USA document containing KLM-R subcompartment data must be classified
        TOP SECRET.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols that contains a token starting with [KLM-R], 
        this rule ensures that attribute @ism:classification is 
        specified with a value containing the token [TS].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M512"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00469</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00469</xsl:attribute>
            <svrl:text>
        [ISM-ID-00469][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains a token starting with [KLM-R], then attribute @ism:disseminationControls must contain
        the name token [NF]. 
        
        Human Readable: A USA document containing KLM-R subcompartment data
        must be marked for NO FOREIGN dissemination.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which specifies
        attribute @ism:SCIcontrols with a token starting with [KLM-R], this rule ensures that
        attribute @ism:disseminationControls is specified with a value containing the token [NF].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M513"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00470</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00470</xsl:attribute>
            <svrl:text>
        [ISM-ID-00470][Error] If ISM_USGOV_RESOURCE and @ism:SCIcontrols contains a
        token matching [KLM-R-XXX], then @ism:disseminationControls cannot contain
        [OC-USGOV]. 
        
        Human Readable: OC-USGOV cannot be used if KLM-R subcompartments are present. 
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE and @ism:SCIcontrols contains
        [KLM-R-XXX], then @ism:disseminationControls cannot contain [OC-USGOV].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M514"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00471</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00471</xsl:attribute>
            <svrl:text>
        [ISM-ID-00471][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains a name token starting with [KLM-R-], then attribute
        @ism:disseminationControls must contain the name token [OC].
        
        Human Readable: A USA document containing KLM-R subcompartment data must be marked for ORIGINATOR CONTROLLED 
        dissemination.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing a token
        starting with [KLM-R-] this rule ensures that attribute
        @ism:disseminationControls is specified with a value containing the token [OC].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M515"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00472</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00472</xsl:attribute>
            <svrl:text>
        [ISM-ID-00472][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains the name token [MVL], then attribute @ism:classification must have
        a value of [TS].
        
        Human Readable: A USA document containing MVL data must be classified
        TOP SECRET.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [MVL], this rule ensures that attribute @ism:classification is 
        specified with a value containing the token [TS].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M516"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00473</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00473</xsl:attribute>
            <svrl:text>
      [ISM-ID-00473][Error] If ISM_USGOV_RESOURCE, PROPIN information (i.e. @ism:disseminationControls of the resource node 
      contains [PR]) requires explicit Foreign Disclosure &amp; Release (FD&amp;R) markings ([REL], [RELIDO], [NF], [DISPLAYONLY] 
      or [EYES]).
   </svrl:text>
            <svrl:text>
      If the document is an ISM_USGOV_RESOURCE, then any element that contains 
      @ism:disseminationControls attribute contains [PR], the document must have one of: [REL], [RELIDO], [NF], [DISPLAYONLY] or [EYES].
   </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M517"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00474</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00474</xsl:attribute>
            <svrl:text>
      [ISM-ID-00474][Warning] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
      contains the name token [HCS], then attribute @ism:SCIcontrols MUST include one of the tokens [HCS-O], [HCS-P] or [HCS-X].
   </svrl:text>
            <svrl:text>
      If the document is an ISM_USGOV_RESOURCE, then for any element that has attribute @ism:SCIcontrols 
      containing the name token [HCS], the element MUST have @ism:SCIcontrols containing have one of: [HCS-O], [HCS-P] or [HCS-X].
   </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M518"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00476</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00476</xsl:attribute>
            <svrl:text> [ISM-ID-00476][Error] If @ism:compliesWith="USA-CUI-ONLY" then attributes
        @ism:SCIcontrols, @ism:SARIdentifier, @ism:atomicEnergyMarkings, @ism:FGIsourceOpen and
        @ism:FGIsourceProtected must not be specified. </svrl:text>
            <svrl:text> If the document has @ism:compliesWith="USA-CUI-ONLY", as defined in
        variable ISM_USCUIONLY_RESOURCE, this rule ensures that NONE of the following attributes are
        specified: @ism:SCIcontrols, @ism:SARIdentifier, @ism:atomicEnergyMarkings,
        @ism:FGIsourceOpen and @ism:FGIsourceProtected . </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M520"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00480</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00480</xsl:attribute>
            <svrl:text>
		Abstract pattern used to warn that an attribute value has a deprecation
		date in its CVE but has not passed based on the ISM_RESOURCE_CREATE_DATE of the resource.
		This pattern uses the deprecation dates in the CVE passed from the calling rule and the
		ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute has a depreciation date,
		which is a warning. The context, CVE name, and Spec name are passed from the calling
		rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M523"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00481</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00481</xsl:attribute>
            <svrl:text>
		Abstract pattern used to warn that an attribute value has a deprecation
		date in its CVE but has not passed based on the ISM_RESOURCE_CREATE_DATE of the resource.
		This pattern uses the deprecation dates in the CVE passed from the calling rule and the
		ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute has a depreciation date,
		which is a warning. The context, CVE name, and Spec name are passed from the calling
		rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M524"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00482</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00482</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that an attribute does not contain a deprecated token. 
		This pattern uses the deprecation dates in the CVE passed from the calling rule and 
		the ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute is deprecated, 
		which is an error. The context, CVE name, and Spec name are passed from the calling rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M525"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00483</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00483</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that an attribute does not contain a deprecated token. 
		This pattern uses the deprecation dates in the CVE passed from the calling rule and 
		the ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute is deprecated, 
		which is an error. The context, CVE name, and Spec name are passed from the calling rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M526"/>
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
            <xsl:attribute name="id">ISM-ID-00487</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00487</xsl:attribute>
            <svrl:text>
		Abstract pattern to enforce that an appropriate notice exists for an
		element in $partTags that has a notice requirement. The calling rule must pass $elem,
		'cuiSpecified', $partTags, and 'FISA'.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M530"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00488</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00488</xsl:attribute>
            <svrl:text>Abstract pattern to ensure that for a given element in an
		ISM_USCUIONLY_RESOURCE with @ism:noticeType containing a specified token and
		ism:externalNotice not equal true, 'FISA' exists in $partCuiSpecified_tok. The calling rule must
		pass 'FISA', 'FISA' and @dataTokenList.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M531"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00491</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00491</xsl:attribute>
            <svrl:text>
		Abstract pattern to enforce that an appropriate notice exists for an
		element in $partTags that has a notice requirement. The calling rule must pass $elem,
		'cuiSpecified', $partTags, and 'SSI'.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M532"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00492</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00492</xsl:attribute>
            <svrl:text>Abstract pattern to ensure that for a given element in an
		ISM_USCUIONLY_RESOURCE with @ism:noticeType containing a specified token and
		ism:externalNotice not equal true, 'SSI' exists in $partCuiSpecified_tok. The calling rule must
		pass 'SSI', 'SSI' and @dataTokenList.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M533"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00495</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00495</xsl:attribute>
            <svrl:text> [ISM-ID-00495][Error] If @ism:compliesWith="USA-CUI-ONLY" then attributes
        @ism:classification and @ism:ownerProducer must not be specified. </svrl:text>
            <svrl:text> If the document has @ism:compliesWith="USA-CUI-ONLY", as defined in
        variable ISM_USCUIONLY_RESOURCE, this rule ensures that NONE of the following attributes are
        specified: @ism:classification and @ism:ownerProducer. </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M535"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00505</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00505</xsl:attribute>
            <svrl:text>
        This abstract pattern checks to see if an attribute of an element exists
        in a list or matches the pattern defined by the list. The calling rule must pass the
        context, search term list, attribute value to check, and an error message.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M545"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00506</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00506</xsl:attribute>
            <svrl:text>
        This abstract pattern checks to see if an attribute of an element exists
        in a list or matches the pattern defined by the list. The calling rule must pass the
        context, search term list, attribute value to check, and an error message.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M546"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00507</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00507</xsl:attribute>
            <svrl:text>
        [ISM-ID-00507][Error] If (ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE) and attribute @ism:disseminationControls
        contains one or more of the name tokens [AC] or [AWP], then attribute @ism:cuiBasic
        must contain the name token [PRIVILEGE].
        
        Human Readable: A CUI document containing one of the CUI limited dissemination controls [AC] or [AWP] must be marked
        with the CUI Basic Category of [PRIVILEGE].
    </svrl:text>
            <svrl:text>
        If the document is an (ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE), for each element which
        specifies attribute @ism:disseminationControls contains one or more of the name tokens [AC] or [AWP], 
        then attribute @ism:cuiBasic must contain the name token [PRIVILEGE].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M547"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00518</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00518</xsl:attribute>
            <svrl:text>
        [ISM-ID-00518][Error] For ism:Notice or ism:NoticeExternal, if @ism:noticeProseID is present then ism:NoticeText is prohibited.
    </svrl:text>
            <svrl:text>
        This rule ensures that for ism:Notice or ism:NoticeExternal, if @ism:noticeProseID is present then ism:NoticeText is prohibited.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M554"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00519</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00519</xsl:attribute>
            <svrl:text>
        [ISM-ID-00519][Error] For ism:Notice or ism:NoticeExternal, if @ism:noticeProseID is absent then ism:NoticeText is required.
    </svrl:text>
            <svrl:text>
        This rule ensures that for ism:Notice or ism:NoticeExternal, if @ism:noticeProseID is absent then ism:NoticeText is required.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M555"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00522</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00522</xsl:attribute>
            <svrl:text> [ISM-ID-00522][Error] If ISM_NSI_EO_APPLIES and the @ism:highWaterNATO
        attribute exists on a banner or portion, then @ism:FGIsourceOpen contains [NATO] or
        @ism:ownerProducer contains [NATO] or @ism:FGIsourceProtected contains [FGI]. Human
        Readable: For documents under E.O. 13526, the NATO high-water indicator can only exist on an
        element where either @ism:FGIsourceOpen contains [NATO] or @ism:ownerProducer contains
        [NATO] or @ism:FGIsourceProtected contains [FGI]. </svrl:text>
            <svrl:text> If ISM_NSI_EO_APPLIES, then for each element which specifies attribute
        @ism:highWaterNATO, this rule ensures that at least one of the attributes @ism:ownerProducer
        or @ism:FGIsourceOpen is specified with a value of [NATO] or @ism:FGIsourceProtected is
        specified with a value of [FGI]. </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M557"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00523</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00523</xsl:attribute>
            <svrl:text> [ISM-ID-00523][Error] If ISM_NSI_EO_APPLIES and the @ism:FGIsourceOpen
        attribute contains [NATO] on a banner or portion, then a requirement exists that @ism:highWaterNATO
        also exists, otherwise the NATO data classification cannot be determined. Human Readable: For documents
        under E.O. 13526, if @ism:FGIsourceOpen contains [NATO], then @ism:highWaterNATO must exist. </svrl:text>
            <svrl:text> If ISM_NSI_EO_APPLIES, then the attribute @ism:highWaterNATO must exist when
        @ism:FGIsourceOpen contains [NATO]. </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M558"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00524</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00524</xsl:attribute>
            <svrl:text> [ISM-ID-00524][Error] If ISM_NSI_EO_APPLIES and the @ism:highWaterNATO
        attribute exists on a banner or portion, then @ism:ownerProducer cannot be equal to 'NATO'.
        Human Readable: For documents under E.O. 13526, the NATO high-water indicator is not allowed
        where @ism:ownerProducer='NATO'. It is okay if @ism:ownerProducer contains 'NATO' and other
        tokens like 'USA'. </svrl:text>
            <svrl:text> If ISM_NSI_EO_APPLIES, then for each element which specifies attribute
        @ism:highWaterNATO, this rule produces an error if @ism:ownerProducer='NATO'. </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M559"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00525</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00525</xsl:attribute>
            <svrl:text> [ISM-ID-00525][Error] If ISM_NSI_EO_APPLIES and the @ism:highWaterNATO
        attribute exists on a banner or portion, then @ism:highWaterNATO cannot be higher than @ism:classification.
        Human Readable: For documents under E.O. 13526, the NATO high-water indicator value cannot be
        higher than the classification value.</svrl:text>
            <svrl:text> If ISM_NSI_EO_APPLIES, then for each element which specifies attribute
        @ism:highWaterNATO, this rule checks the value of @ism:classification.  If the value of @ism:highWaterNATO
        is 'NATO-TS' then @ism:classification must be 'TS'. If the value of @ism:highWaterNATO is 'NATO-S',
        then @ism:classification must be 'TS' or 'S'. If the value of @ism:highWaterNATO is 'NATO-C', then
        @ism:classification must be 'C', 'S' or 'TS'.  If the value of @ism:highWaterNATO is 'NATO-R' then
        @ism:classification cannot be 'U'.  If the value of @ism:highWaterNATO is 'NATO-U' then any value
        of classification is ok.  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M560"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00526</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00526</xsl:attribute>
            <svrl:text> [ISM-ID-00526][Error] If ISM_NSI_EO_APPLIES and the @ism:ownerProducer
        attribute contains multiple values on a banner or portion, one being NATO, then a requirement exists that @ism:highWaterNATO
        also exists, otherwise the NATO data classification cannot be determined. Human Readable: For documents
        under E.O. 13526, if @ism:ownerProducer attribute contains multiple values and NATO, then @ism:highWaterNATO must exist. </svrl:text>
            <svrl:text> If ISM_NSI_EO_APPLIES, then the attribute @ism:highWaterNATO must exist when
        @ism:ownerProducer attribute contains multiple values and NATO. </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M561"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00532</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00532</xsl:attribute>
            <svrl:text> [ISM-ID-00532][Error] For all elements with @ism:SARIdentifier with tokens
        that include classification portion marks (e.g., DOD:TS:aaaa or DOD:C:bbbb), the value of
        the classification portion mark cannot be higher than @ism:classification on the same
        element. Human Readable: For @ism:SARIdentifier tokens that include classification portion
        marks in their values, the classification portion mark cannot be higher than the
        classification value. Note that some @ism:SARIdentifier tokens may not contain
        classification portion marks, e.g., DNI:kkkk; the rule does not apply to these tokens. </svrl:text>
            <svrl:text> For all elements with @ism:SARIdentifier with tokens that include
        classification portion marks (e.g., DOD:TS:aaaa or DOD:C:bbbb), check the value of the
        classification portion mark, which is found between two colons ':' according to the regex
        for SARs. The logic uses the fact that if, for example, ':TS:' is found anywhere in
        @ism:SARIdentifier, then the classification of the element should be 'TS'. The rule logic is
        as follows. If there is ':TS:' the @ism:SARIdentifier, then @ism:classification must be
        'TS'. Otherwise, if there is ':S:' in the @ism:SARIdentifier, then @ism:classification must
        be 'S' or 'TS'. Otherwise, if there is ':C:' in the @ism:SARIdentifier, then
        @ism:classification must be 'C' or 'S' or 'TS'. Otherwise, according to the regex, there
        is no classification portion marking in any of the tokens in @ism:SARIdentifier, so do not check against @ism:classification. 
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M566"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00535</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00535</xsl:attribute>
            <svrl:text>
        [ISM-ID-00535][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:disseminationControls contains the name token [WAIVED], then 
        attribute @ism:compliesWith must contain [USDOD]. 
        Human Readable: USA documents containing the WAIVED dissemination control must comply with USDOD rules.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [WAIVED], this rule ensures that attribute @ism:compliesWith contains [USDOD].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M569"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00119</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00119</xsl:attribute>
            <svrl:text>
        [ISM-ID-00119][Error] If ISM_USIC_RESOURCE and 
        1. attribute @ism:classification is not [U]
        AND
        2. not ISM_710_FDR_EXEMPT
        AND
        3. attribute @ism:excludeFromRollup is not true
        AND
        4. attribute @ism:disseminationControls must contain one or more of 
            [DISPLAYONLY], [REL], [RELIDO], [EYES], or [NF].
        
        Human Readable: All classified NSI that does not claim exemption from
        ICD 710 mandatory Foreign Disclosure and Release must have an 
        appropriate foreign disclosure or release marking.
    </svrl:text>
            <svrl:text>
        If IC Markings System Register and Manual rules do not apply to the document, or the document is exempt from mandatory
        foreign disclosure and release markings, or the resource is unclassified, or excludeFromRollup is true, 
        then the rule does not apply. Otherwise, this rule ensures that the attribute disseminationControls contains at least
        one of the values [DISPLAYONLY], [RELIDO], [REL], [EYES], or [NF].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M570"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00225</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00225</xsl:attribute>
            <svrl:text>
        [ISM-ID-00225][Error] If subject to IC rules, then attribute @ism:nonICmarkings must NOT be specified 
        with a value containing any name token starting with [ACCM] or [NNPI]. 
        
        Human Readable: ACCM and NNPI tokens are not valid for documents that are subject to IC rules.
    </svrl:text>
            <svrl:text>
        If ISM_USIC_RESOURCE, for each element which has attribute @ism:nonICmarkings specified, this rule ensures that
        attribute @ism:nonICmarkings is not specified with a value containing a token which starts with [ACCM] or [NNPI].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M571"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00251</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00251</xsl:attribute>
            <svrl:text>
        [ISM-ID-00251][Error] If ISM_USIC_RESOURCE, then attribute @ism:noticeType must not be specified with a value of [COMSEC]. 
        
        Human Readable: COMSEC notices are not valid for US IC documents.
    </svrl:text>
            <svrl:text>
    	If ISM_USIC_RESOURCE, for each element which has attribute @ism:noticeType specified, this rule ensures 
    	that attribute @ism:noticeType is not specified with a value containing token [COMSEC].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M572"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00002</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00002</xsl:attribute>
            <svrl:text>
        [ISM-ID-00002][Error] For every attribute in the ISM namespace that is used in a document, a non-null value must be present.
    </svrl:text>
            <svrl:text>
        For each element which defines an attribute in the ISM namespace, this rule ensures that each attribute in the ISM namespace 
        is specified with a non-whitespace value.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M573"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00012</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00012</xsl:attribute>
            <svrl:text> 
        [ISM-ID-00012][Error] If the document is not USA-CUI-ONLY, AND: 
        1. any of the attributes defined in this DES other than @ism:DESVersion, @ism:ISMCATCESVersion,
        @ism:unregisteredNoticeType, or @ism:pocType are specified for an element, 
        OR
        2. the current node is one of elements arh:Security, arh:ExternalSecurity, ntk:Access or ntk:AccessProfile,
        then attributes @ism:classification and @ism:ownerProducer must be specified for the element.</svrl:text>
            <svrl:text> If the document does NOT have @ism:compliesWith="USA-CUI-ONLY", then for
        each element which defines an attribute in the ISM namespace other than @ism:pocType,
        @ism:DESVersion, @ism:ISMCATCESVersion, or @ism:unregisteredNoticeType, or the element is arh:Security,
        or arh:ExternalSecurity or ntk:Access or ntk:AccessProfile, this rule ensures that
        attributes @ism:classification and @ism:ownerProducer are specified. </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M574"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00102</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00102</xsl:attribute>
            <svrl:text>
        [ISM-ID-00102][Error] The attribute @ism:DESVersion in the namespace urn:us:gov:ic:ism must be specified.   
        
        Human Readable: The data encoding specification version must be specified.
    </svrl:text>
            <svrl:text>
        Make sure that the attribute @ism:DESVersion is specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M575"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00103</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00103</xsl:attribute>
            <svrl:text>
        [ISM-ID-00103][Error] At least one element must have attribute @ism:resourceElement specified with a value of [true].
    </svrl:text>
            <svrl:text>
        For the document, this rule ensures that at least one element specifies attribute @ism:resourceElement with a value of [true].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M576"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00163</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00163</xsl:attribute>
            <svrl:text>
        [ISM-ID-00163][Error] If attribute @ism:nonUSControls exists either 
        1. the attribute @ism:ownerProducer must equal [NATO] or a [NATO:NAC] 
            OR 
        2. the attribute @ism:FGIsourceOpen must contain [NATO] or a [NATO:NAC]
            OR
        3. the attribute @ism:FGIsourceProtected is used (This should only be the case when it is a resource level or super portion marking)
        
        Human Readable: NATO and NATO/NACs are the only owner of classification markings for which nonUSControls are currently authorized.
    </svrl:text>
            <svrl:text>
        For each element which specifies attribute @ism:nonUSControls, this rule ensures that either the attributes 
        @ism:ownerProducer or @ism:FGIsourceOpen are specified with a value of [NATO] or [NATO:NAC]
        OR the @ism:FGIsourceProtected attribute is specified. </svrl:text>
            <svrl:text>        
        NOTE: The last case with @ism:FGIsourceProtected should only occur when the element is either a resource node or 
        a super-portion such as the marking of a table where the table contains one or more portions meeting 1 or 2 from the rule description 
        AND one or more portions with @ism:FGIsourceProtected is specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M579"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00194</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00194</xsl:attribute>
            <svrl:text>
		Abstract pattern used to warn that an attribute value has a deprecation
		date in its CVE but has not passed based on the ISM_RESOURCE_CREATE_DATE of the resource.
		This pattern uses the deprecation dates in the CVE passed from the calling rule and the
		ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute has a depreciation date,
		which is a warning. The context, CVE name, and Spec name are passed from the calling
		rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M580"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00195</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00195</xsl:attribute>
            <svrl:text>
		Abstract pattern to ensure that an attribute does not contain a deprecated token. 
		This pattern uses the deprecation dates in the CVE passed from the calling rule and 
		the ISM_RESOURCE_CREATE_DATE to determine if a token in the attribute is deprecated, 
		which is an error. The context, CVE name, and Spec name are passed from the calling rule.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M581"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00376</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00376</xsl:attribute>
            <svrl:text>
	  	[ISM-ID-00376][Error] A portion using tetragraphs may not have a releasableTo 
	  	that is less restrictive than the releasability of any tetragraph or organization tokens used
	  	in the same portion’s releasableTo, displayOnlyTo, FGIsourceOpen, or FGIsourceProtected attributes.
	  	If a tetragraph XXXX in any of the attributes ownerProducer, releasableTo, displayOnlyTo, FGIsourceOpen, 
	  	or FGIsourceProtected is itself marked as ism:releasableTo in the Tetragraph Taxonomy, then see if all
	  	the countries that the portion is releasableTo are also countries that the tetragraph XXXX is releasableTo.  If not, error.  
	</svrl:text>
            <svrl:text>
	  	Determine the set of releasableTo countries by determining, for each token in releasableTo, if it is a country code or tetragraph.
	  	If it is a tetragraph get the membership from ISMCAT Taxonomy and add the membership to the variable releasableToCountries; 
	  	otherwise, add the token to the variable releasableToCountries.  Then get the list of tetragraphs that appear in any of the 
	  	attributes @ism:ownerProducer, @ism:releasableTo, @ism:displayOnlyTo, @ism:FGIsourceOpen, or @ism:FGIsourceProtected and 
	  	put that list into the variable myTetras. Then determine if any of the tetragraph tokens in myTetras have releasability restrictions 
	  	themselves. If so, add those tetragraphs to the variable tetrasWithReleasableTo. Finally, determine if the releasability of any of the 
	  	tetragraph tokens in tetrasWithReleasableTo is more restrictive then the releasability of the portion, and if so,
	  	trigger the error message.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M594"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00377</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00377</xsl:attribute>
            <svrl:text>
        This abstract pattern checks to see if an attribute of an element exists
        in a list or matches the pattern defined by the list. The calling rule must pass the
        context, search term list, attribute value to check, and an error message.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M595"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00382</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00382</xsl:attribute>
            <svrl:text>
		[ISM-ID-00382][Error] For all elements with single-valued @ism:ownerProducer, @ism:joint must NOT be true.
	</svrl:text>
            <svrl:text>
		For all elements whose count of @ism:ownerProducer token values is equal to 1, @ism:joint must NOT be set to true.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M598"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00383</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00383</xsl:attribute>
            <svrl:text>
		[ISM-ID-00383][Error] For elements with @ism:joint set to true, one of the values of @ism:ownerProducer must be USA.
	</svrl:text>
            <svrl:text>
		For elements with @ism:joint set to true, one of the values of @ism:ownerProducer must be USA.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M599"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00453</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00453</xsl:attribute>
            <svrl:text>
        [ISM-ID-00453][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains a token matching [HCS-P-XXXXXX], 
        where X is represented by the regular expression character class [A-Z0-9]{1,6}, then attribute
        @ism:classification must have a value of [TS].
        
        Human Readable: A USA document with HCS-PRODUCT subcompartment data must be classified TOP SECRET.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing a token matching
        [HCS-P-XXXXXX], where X is represented by the regular expression character
        class [A-Z0-9]{1,6}, this rule ensures that attribute @ism:classification is 
        specified with a value containing the token [TS].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M608"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00511</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00511</xsl:attribute>
            <svrl:text>
        [ISM-ID-00511][Error] arh:Security/@ism:resourceElement attribute must be true.
        
        Human Readable: arh:Security element must contain @ism:resourceElement attribute and @ism:resourceElement
        must equal 'true'.
    </svrl:text>
            <svrl:text>
        Find each instance of arh:Security in the document, test that it has @ism:resourceElement.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M610"/>
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

   <!--PATTERN ISM-ID-00157-->


	<!--RULE ISM-ID-00157-R1-->
<xsl:template match="*[$ISM_USDOD_RESOURCE and util:containsAnyOfTheTokens(@ism:noticeType, ('DoD-Dist-B', 'DoD-Dist-C', 'DoD-Dist-D', 'DoD-Dist-E'))]"
                 priority="1000"
                 mode="M246">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USDOD_RESOURCE and util:containsAnyOfTheTokens(@ism:noticeType, ('DoD-Dist-B', 'DoD-Dist-C', 'DoD-Dist-D', 'DoD-Dist-E'))]"
                       id="ISM-ID-00157-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:noticeReason"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:noticeReason">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00157][Error] If ISM_USDOD_RESOURCE and: 
            1. The attribute notice contains one of the [DoD-Dist-B], [DoD-Dist-C], [DoD-Dist-D], or [DoD-Dist-E] 
            AND
            2. The attribute @ism:noticeReason is not specified. 
            
            Human Readable: DoD distribution statements B, C, D , or E all require a reason. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M246"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M246"/>
   <xsl:template match="@*|node()" priority="-2" mode="M246">
      <xsl:apply-templates select="*" mode="M246"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00161-->


	<!--RULE ISM-ID-00161-R1-->
<xsl:template match="*[$ISM_USDOD_RESOURCE and (util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:noticeType, ('DoD-Dist-A'))) and not (@ism:excludeFromRollup=true())]"
                 priority="1000"
                 mode="M248">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USDOD_RESOURCE and (util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:noticeType, ('DoD-Dist-A'))) and not (@ism:excludeFromRollup=true())]"
                       id="ISM-ID-00161-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:nonICmarkings)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="not(@ism:nonICmarkings)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> 
            [ISM-ID-00161][Error] If the document is an
            1. ISM_USDOD_RESOURCE AND
            2. the attribute @ism:noticeType of ISM_RESOURCE_ELEMENT contains [DoD-Dist-A] AND
            3. no portions in the document have their attribute @ism:excludeFromRollup set to [true]
            THEN there must not be any attribute @ism:nonICmarkings present.
            
            Human Readable: Distribution statement A (Public Release) is 
            incompatible with any nonICMarkings if excludeFromRollup is not TRUE.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M248"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M248"/>
   <xsl:template match="@*|node()" priority="-2" mode="M248">
      <xsl:apply-templates select="*" mode="M248"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00237-->


	<!--RULE ISM-ID-00237-R1-->
<xsl:template match="*[$ISM_USDOD_RESOURCE and util:containsAnyOfTheTokens(@ism:noticeType, ('DoD-Dist-B', 'DoD-Dist-C', 'DoD-Dist-D', 'DoD-Dist-E', 'DoD-Dist-F'))]"
                 priority="1000"
                 mode="M251">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USDOD_RESOURCE and util:containsAnyOfTheTokens(@ism:noticeType, ('DoD-Dist-B', 'DoD-Dist-C', 'DoD-Dist-D', 'DoD-Dist-E', 'DoD-Dist-F'))]"
                       id="ISM-ID-00237-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:noticeDate"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:noticeDate">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> 
            [ISM-ID-00237][Error] If ISM_USDOD_RESOURCE, any element which specifies
            attribute @ism:noticeType containing one of the tokens [DoD-Dist-B], 
            [DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F]
            must also specify attribute @ism:noticeDate.     	
            
            Human Readable: DoD distribution statements B, C, D, E, and F all require a date.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M251"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M251"/>
   <xsl:template match="@*|node()" priority="-2" mode="M251">
      <xsl:apply-templates select="*" mode="M251"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00238-->


	<!--RULE ISM-ID-00238-R1-->
<xsl:template match="*[$ISM_USDOD_RESOURCE and util:containsAnyOfTheTokens(@ism:noticeType, ('DoD-Dist-B', 'DoD-Dist-C', 'DoD-Dist-D', 'DoD-Dist-E', 'DoD-Dist-F'))]"
                 priority="1000"
                 mode="M252">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USDOD_RESOURCE and util:containsAnyOfTheTokens(@ism:noticeType, ('DoD-Dist-B', 'DoD-Dist-C', 'DoD-Dist-D', 'DoD-Dist-E', 'DoD-Dist-F'))]"
                       id="ISM-ID-00238-R1"/>
      <xsl:variable name="foundNoticeTokens"
                    select="for $noticeToken in tokenize(normalize-space(string(@ism:noticeType)), ' ') return if(matches($noticeToken, '^DoD-Dist-[BCDEF]')) then $noticeToken else null"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $noticeToken in $foundNoticeTokens satisfies index-of($partPocType_tok, $noticeToken)&gt;0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $noticeToken in $foundNoticeTokens satisfies index-of($partPocType_tok, $noticeToken)&gt;0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> 
            [ISM-ID-00238][Error] If ISM_USDOD_RESOURCE, if any element specifies
            attribute @ism:noticeType containing one of the tokens [DoD-Dist-B], 
            [DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F]
            then an element in the document must specify attribute @ism:pocType with
            the same value as attribute @ism:noticeType.
            
            Human Readable: DoD distribution statements B, C, D, E, and F all 
            require a corresponding point of contact.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M252"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M252"/>
   <xsl:template match="@*|node()" priority="-2" mode="M252">
      <xsl:apply-templates select="*" mode="M252"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00016-->


	<!--RULE ISM-ID-00016-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and @ism:classification='U']"
                 priority="1000"
                 mode="M257">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and @ism:classification='U']"
                       id="ISM-ID-00016-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:classificationReason or @ism:classifiedBy or @ism:declassDate or @ism:declassEvent or @ism:declassException or @ism:derivativelyClassifiedBy or @ism:derivedFrom or @ism:SARIdentifier or @ism:SCIcontrols)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(@ism:classificationReason or @ism:classifiedBy or @ism:declassDate or @ism:declassEvent or @ism:declassException or @ism:derivativelyClassifiedBy or @ism:derivedFrom or @ism:SARIdentifier or @ism:SCIcontrols)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00016][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:classification has a value of [U], then attributes @ism:classificationReason,
            @ism:classifiedBy, @ism:derivativelyClassifiedBy, @ism:declassDate, @ism:declassEvent, 
            @ism:declassException, @ism:derivedFrom, @ism:SARIdentifier, or @ism:SCIcontrols must not be specified.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M257"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M257"/>
   <xsl:template match="@*|node()" priority="-2" mode="M257">
      <xsl:apply-templates select="*" mode="M257"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00017-->


	<!--RULE ISM-ID-00017-R1-->
<xsl:template match="*[$ISM_NSI_EO_APPLIES and @ism:classifiedBy]"
                 priority="1000"
                 mode="M258">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_NSI_EO_APPLIES and @ism:classifiedBy]"
                       id="ISM-ID-00017-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:classificationReason"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="@ism:classificationReason">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00017][Error] If ISM_NSI_EO_APPLIES and attribute 
            @ism:classifiedBy is specified, then attribute @ism:classificationReason must be specified.         
            Human Readable: Documents under E.O. 13526 containing Originally Classified data require a
            classification reason to be identified.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M258"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M258"/>
   <xsl:template match="@*|node()" priority="-2" mode="M258">
      <xsl:apply-templates select="*" mode="M258"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00028-->


	<!--RULE ISM-ID-00028-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC', 'EYES'))]"
                 priority="1000"
                 mode="M259">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC', 'EYES'))]"
                       id="ISM-ID-00028-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:classification=('TS', 'S', 'C')"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="@ism:classification=('TS', 'S', 'C')">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00028][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls contains the name token [OC] or [EYES],
            then attribute @ism:classification must have a value of [TS], [S], or [C].
            Human Readable: Portions marked for ORCON or EYES ONLY dissemination 
            in a USA document must be CONFIDENTIAL, SECRET, or TOP SECRET.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M259"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M259"/>
   <xsl:template match="@*|node()" priority="-2" mode="M259">
      <xsl:apply-templates select="*" mode="M259"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00030-->


	<!--RULE ISM-ID-00030-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('FOUO'))]"
                 priority="1000"
                 mode="M260">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('FOUO'))]"
                       id="ISM-ID-00030-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:classification='U'"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:classification='U'">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00030][Error] If ISM_USGOV_RESOURCE and attribute @ism:disseminationControls contains the name token [FOUO], 
            then attribute @ism:classification must have a value of [U].
            Human Readable: Portions marked for FOUO dissemination in a USA document
            must be classified UNCLASSIFIED.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M260"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M260"/>
   <xsl:template match="@*|node()" priority="-2" mode="M260">
      <xsl:apply-templates select="*" mode="M260"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00031-->


	<!--RULE ISM-ID-00031-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('REL', 'EYES'))]"
                 priority="1000"
                 mode="M261">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('REL', 'EYES'))]"
                       id="ISM-ID-00031-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:releasableTo"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:releasableTo">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00031][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls contains the name token [REL] or [EYES], then 
            attribute @ism:releasableTo must be specified. 
            Human Readable: USA documents containing REL TO or EYES ONLY 
            dissemination must specify to which countries the document is releasable.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M261"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M261"/>
   <xsl:template match="@*|node()" priority="-2" mode="M261">
      <xsl:apply-templates select="*" mode="M261"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00032-->


	<!--RULE ISM-ID-00032-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('REL', 'EYES')))]"
                 priority="1000"
                 mode="M262">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('REL', 'EYES')))]"
                       id="ISM-ID-00032-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:releasableTo)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="not(@ism:releasableTo)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00032][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls is not specified, or is specified and does not 
            contain the name token [REL] or [EYES], then attribute @ism:releasableTo 
            must not be specified.
            
            Human Readable: USA documents must only specify to which countries it is 
            authorized for release if dissemination information contains 
            REL TO or EYES ONLY data. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M262"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M262"/>
   <xsl:template match="@*|node()" priority="-2" mode="M262">
      <xsl:apply-templates select="*" mode="M262"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00033-->


	<!--RULE MutuallyExclusiveAttributeValues-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('REL', 'EYES', 'NF'))]"
                 priority="1000"
                 mode="M263">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('REL', 'EYES', 'NF'))]"
                       id="MutuallyExclusiveAttributeValues-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( for $token in tokenize(normalize-space(string(@ism:disseminationControls)),' ') return  if($token = ('REL', 'EYES', 'NF')) then 1 else null ) = 1"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( for $token in tokenize(normalize-space(string(@ism:disseminationControls)),' ') return if($token = ('REL', 'EYES', 'NF')) then 1 else null ) = 1">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			               <xsl:text/>
                  <xsl:value-of select="'   [ISM-ID-00033][Error] If ISM_USGOV_RESOURCE, then tokens [REL], [EYES]    and [NF] are mutually exclusive for attribute disseminationControls.   '"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M263"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M263"/>
   <xsl:template match="@*|node()" priority="-2" mode="M263">
      <xsl:apply-templates select="*" mode="M263"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00038-->


	<!--RULE MutuallyExclusiveAttributeValues-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('XD', 'ND', 'SBU', 'SBU-NF'))]"
                 priority="1000"
                 mode="M265">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('XD', 'ND', 'SBU', 'SBU-NF'))]"
                       id="MutuallyExclusiveAttributeValues-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( for $token in tokenize(normalize-space(string(@ism:nonICmarkings)),' ') return  if($token = ('XD', 'ND', 'SBU', 'SBU-NF')) then 1 else null ) = 1"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( for $token in tokenize(normalize-space(string(@ism:nonICmarkings)),' ') return if($token = ('XD', 'ND', 'SBU', 'SBU-NF')) then 1 else null ) = 1">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			               <xsl:text/>
                  <xsl:value-of select="'   [ISM-ID-00038][Error] If ISM_USGOV_RESOURCE, then the tokens    [XD], [ND], [SBU], and [SBU-NF] are mutually exclusive for attribute nonICmarkings.      Human Readable: USA documents must not specify [XD], [ND], [SBU], and/or [SBU-NF] commingled on a single element.   '"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M265"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M265"/>
   <xsl:template match="@*|node()" priority="-2" mode="M265">
      <xsl:apply-templates select="*" mode="M265"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00040-->


	<!--RULE ValidateValueExistenceInList-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:ownerProducer, ('USA'))]"
                 priority="1000"
                 mode="M266">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:ownerProducer, ('USA'))]"
                       id="ValidateValueExistenceInList-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $token in $classificationUSList satisfies $token = @ism:classification"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $token in $classificationUSList satisfies $token = @ism:classification">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'   [ISM-ID-00040][Error] If ISM_USGOV_RESOURCE and attribute ownerProducer contains [USA] then attribute classification must have a value in CVEnumISMClassificationUS.xml.   '"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M266"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M266"/>
   <xsl:template match="@*|node()" priority="-2" mode="M266">
      <xsl:apply-templates select="*" mode="M266"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00043-->


	<!--RULE ISM-ID-00043-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('SI'))]"
                 priority="1000"
                 mode="M267">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('SI'))]"
                       id="ISM-ID-00043-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S', 'C'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S', 'C'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00043][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
            contains the name token [SI], then attribute @ism:classification must have
            a value of [TS], [S], or [C].
            
            Human Readable: A USA document containing Special Intelligence (SI) 
            data must be classified CONFIDENTIAL, SECRET, or TOP SECRET.  
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M267"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M267"/>
   <xsl:template match="@*|node()" priority="-2" mode="M267">
      <xsl:apply-templates select="*" mode="M267"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00044-->


	<!--RULE ISM-ID-00044-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^SI-G$'))]"
                 priority="1000"
                 mode="M268">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^SI-G$'))]"
                       id="ISM-ID-00044-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:classification, ('TS'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:classification, ('TS'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00044][Error] If the document is an ISM_USGOV_RESOURCE and the
            attribute @ism:SCIcontrols contain a name token with [SI-G], then the attribute @ism:classification
            must have a value of [TS]. 
            
            Human Readable: A USA document containing Special Intelligence (SI) GAMMA compartment data 
            must be classified TOP SECRET. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M268"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M268"/>
   <xsl:template match="@*|node()" priority="-2" mode="M268">
      <xsl:apply-templates select="*" mode="M268"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00045-->


	<!--RULE ISM-ID-00045-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^SI-G$'))]"
                 priority="1000"
                 mode="M269">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^SI-G$'))]"
                       id="ISM-ID-00045-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
          [ISM-ID-00045][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
          contains a name token starting with [SI-G], then attribute
          @ism:disseminationControls must contain the name token [OC].
          
          Human Readable: A USA document containing Special Intelligence (SI)
          GAMMA compartment data must be marked for ORIGINATOR CONTROLLED 
          dissemination.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M269"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M269"/>
   <xsl:template match="@*|node()" priority="-2" mode="M269">
      <xsl:apply-templates select="*" mode="M269"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00047-->


	<!--RULE ISM-ID-00047-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('TK'))]"
                 priority="1000"
                 mode="M270">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('TK'))]"
                       id="ISM-ID-00047-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00047][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
            contains the name token [TK], then attribute @ism:classification must have
            a value of [TS] or [S].
            
            Human Readable: A USA document containing TALENT KEYHOLE data must
            be classified SECRET or TOP SECRET.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M270"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M270"/>
   <xsl:template match="@*|node()" priority="-2" mode="M270">
      <xsl:apply-templates select="*" mode="M270"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00048-->


	<!--RULE ISM-ID-00048-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS'))]"
                 priority="1000"
                 mode="M271">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS'))]"
                       id="ISM-ID-00048-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00048][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
            contains the name token [HCS], then attribute @ism:classification must have
            a value of [TS] or [S].
            
            Human Readable: A USA document containing HCS data must be classified
            SECRET or TOP SECRET.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M271"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M271"/>
   <xsl:template match="@*|node()" priority="-2" mode="M271">
      <xsl:apply-templates select="*" mode="M271"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00049-->


	<!--RULE ISM-ID-00049-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS'))]"
                 priority="1000"
                 mode="M272">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS'))]"
                       id="ISM-ID-00049-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
              [ISM-ID-00049][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
              contains the name token [HCS], then attribute @ism:disseminationControls
              must contain the name token [NF].
              
              Human Readable: A USA document containing HCS data must be marked
              for NO FOREIGN dissemination.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M272"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M272"/>
   <xsl:template match="@*|node()" priority="-2" mode="M272">
      <xsl:apply-templates select="*" mode="M272"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00097-->


	<!--RULE ISM-ID-00097-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and @ism:FGIsourceProtected]"
                 priority="1000"
                 mode="M298">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and @ism:FGIsourceProtected]"
                       id="ISM-ID-00097-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="normalize-space(string(./@ism:FGIsourceProtected))='FGI'"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="normalize-space(string(./@ism:FGIsourceProtected))='FGI'">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00097][Warning] If ISM_USGOV_RESOURCE and attribute @ism:FGIsourceProtected is 
            specified with a value other than [FGI] then the value(s) must not be discoverable in IC shared spaces.
            
            Human Readable: FGI Protected should rarely if ever be seen outside of an agency's internal systems. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M298"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M298"/>
   <xsl:template match="@*|node()" priority="-2" mode="M298">
      <xsl:apply-templates select="*" mode="M298"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00099-->


	<!--RULE ISM-ID-00099-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:ownerProducer, ('FGI'))]"
                 priority="1000"
                 mode="M299">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:ownerProducer, ('FGI'))]"
                       id="ISM-ID-00099-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count(tokenize(normalize-space(string(@ism:ownerProducer)), ' ')) = 1"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count(tokenize(normalize-space(string(@ism:ownerProducer)), ' ')) = 1">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00099][Error] If ISM_USGOV_RESOURCE and attribute @ism:ownerProducer
            contains the token [FGI], then the token [FGI] must be the only value in attribute @ism:ownerProducer.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M299"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M299"/>
   <xsl:template match="@*|node()" priority="-2" mode="M299">
      <xsl:apply-templates select="*" mode="M299"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00107-->


	<!--RULE ISM-ID-00107-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('IMC'))]"
                 priority="1000"
                 mode="M302">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('IMC'))]"
                       id="ISM-ID-00107-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:classification=('TS', 'S')"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="@ism:classification=('TS', 'S')">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00107][Error] If ISM_USGOV_RESOURCE and attribute @ism:disseminationControls contains the name token [IMC] 
            then attribute @ism:classification must have a value of [TS] or [S].
            
            Human Readable: IMCON data is SECRET (S), but may appear with S or TOP SECRET data.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M302"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M302"/>
   <xsl:template match="@*|node()" priority="-2" mode="M302">
      <xsl:apply-templates select="*" mode="M302"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00124-->


	<!--RULE ISM-ID-00124-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('RELIDO'))]"
                 priority="1000"
                 mode="M306">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('RELIDO'))]"
                       id="ISM-ID-00124-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:ownerProducer, ('USA'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:ownerProducer, ('USA'))">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
          [ISM-ID-00124][Warning] If ISM_USGOV_RESOURCE and
          1. Attribute @ism:ownerProducer does not contain [USA].
          AND
          2. Attribute @ism:disseminationControls contains [RELIDO]
          
          Human Readable: RELIDO is not authorized for non-US portions.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M306"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M306"/>
   <xsl:template match="@*|node()" priority="-2" mode="M306">
      <xsl:apply-templates select="*" mode="M306"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00127-->


	<!--RULE DataHasCorrespondingNotice-R1-->
<xsl:template match="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD'))]"
                 priority="1000"
                 mode="M307">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD'))]"
                       id="DataHasCorrespondingNotice-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('RD')) and not ($elem/@ism:externalNotice=true()))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('RD')) and not ($elem/@ism:externalNotice=true()))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00127'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE, any
			element meeting ISM_CONTRIBUTES in the document has the attribute <xsl:text/>
                  <xsl:value-of select="'atomicEnergyMarkings'"/>
                  <xsl:text/> containing [<xsl:text/>
                  <xsl:value-of select="'RD'"/>
                  <xsl:text/>], then some
			element meeting ISM_CONTRIBUTES in the document MUST have attribute noticeType
			containing [<xsl:text/>
                  <xsl:value-of select="'RD'"/>
                  <xsl:text/>].</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M307"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M307"/>
   <xsl:template match="@*|node()" priority="-2" mode="M307">
      <xsl:apply-templates select="*" mode="M307"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00128-->


	<!--RULE DataHasCorrespondingNoticeWithException-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings, ('FRD')) and not(util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings, ('RD')))]"
                 priority="1000"
                 mode="M308">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings, ('FRD')) and not(util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings, ('RD')))]"
                       id="DataHasCorrespondingNoticeWithException-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('FRD')) and not($elem/@ism:externalNotice = true()))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('FRD')) and not($elem/@ism:externalNotice = true()))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00128'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE, any
			element meeting ISM_CONTRIBUTES in the document has the attribute <xsl:text/>
                  <xsl:value-of select="'atomicEnergyMarkings'"/>
                  <xsl:text/> containing [<xsl:text/>
                  <xsl:value-of select="'FRD'"/>
                  <xsl:text/>], then some
			element meeting ISM_CONTRIBUTES in the document MUST have attribute noticeType
			containing [<xsl:text/>
                  <xsl:value-of select="'FRD'"/>
                  <xsl:text/>].</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M308"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M308"/>
   <xsl:template match="@*|node()" priority="-2" mode="M308">
      <xsl:apply-templates select="*" mode="M308"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00129-->


	<!--RULE DataHasCorrespondingNotice-R1-->
<xsl:template match="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('IMC'))]"
                 priority="1000"
                 mode="M309">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('IMC'))]"
                       id="DataHasCorrespondingNotice-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('IMC', 'IMCON_RSEN')) and not ($elem/@ism:externalNotice=true()))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('IMC', 'IMCON_RSEN')) and not ($elem/@ism:externalNotice=true()))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00129'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE, any
			element meeting ISM_CONTRIBUTES in the document has the attribute <xsl:text/>
                  <xsl:value-of select="'disseminationControls'"/>
                  <xsl:text/> containing [<xsl:text/>
                  <xsl:value-of select="'IMC'"/>
                  <xsl:text/>], then some
			element meeting ISM_CONTRIBUTES in the document MUST have attribute noticeType
			containing [<xsl:text/>
                  <xsl:value-of select="'IMC', 'IMCON_RSEN'"/>
                  <xsl:text/>].</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M309"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M309"/>
   <xsl:template match="@*|node()" priority="-2" mode="M309">
      <xsl:apply-templates select="*" mode="M309"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00130-->


	<!--RULE DataHasCorrespondingNotice-R1-->
<xsl:template match="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('FISA'))]"
                 priority="1000"
                 mode="M310">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('FISA'))]"
                       id="DataHasCorrespondingNotice-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('FISA')) and not ($elem/@ism:externalNotice=true()))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('FISA')) and not ($elem/@ism:externalNotice=true()))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00130'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE, any
			element meeting ISM_CONTRIBUTES in the document has the attribute <xsl:text/>
                  <xsl:value-of select="'disseminationControls'"/>
                  <xsl:text/> containing [<xsl:text/>
                  <xsl:value-of select="'FISA'"/>
                  <xsl:text/>], then some
			element meeting ISM_CONTRIBUTES in the document MUST have attribute noticeType
			containing [<xsl:text/>
                  <xsl:value-of select="'FISA'"/>
                  <xsl:text/>].</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M310"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M310"/>
   <xsl:template match="@*|node()" priority="-2" mode="M310">
      <xsl:apply-templates select="*" mode="M310"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00133-->


	<!--RULE ISM-ID-00133-R1-->
<xsl:template match="*[$ISM_NSI_EO_APPLIES and util:containsAnyOfTheTokens(@ism:declassException, ('25X1-EO-12951', '50X1-HUM', '50X2-WMD', 'NATO', 'AEA', 'NATO-AEA'))]"
                 priority="1000"
                 mode="M312">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_NSI_EO_APPLIES and util:containsAnyOfTheTokens(@ism:declassException, ('25X1-EO-12951', '50X1-HUM', '50X2-WMD', 'NATO', 'AEA', 'NATO-AEA'))]"
                       id="ISM-ID-00133-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:declassDate or @ism:declassEvent)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(@ism:declassDate or @ism:declassEvent)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[ISM-ID-00133][Error] If ISM_NSI_EO_APPLIES and attribute 
			@ism:declassException is specified and contains the tokens [25X1-EO-12951],
			[50X1-HUM], [50X2-WMD], [NATO], [AEA] or [NATO-AEA] 
			then attribute @ism:declassDate or @ism:declassEvent must NOT be specified.
			
			Human Readable: Documents under E.O. 13526 must not specify declassDate or declassEvent if 
			a declassException of 25X1-EO-12951, 50X1-HUM, 50X2-WMD, NATO, AEA or NATO-AEA is specified.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M312"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M312"/>
   <xsl:template match="@*|node()" priority="-2" mode="M312">
      <xsl:apply-templates select="*" mode="M312"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00134-->


	<!--RULE DataHasCorrespondingNotice-R1-->
<xsl:template match="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('DS'))]"
                 priority="1000"
                 mode="M313">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('DS'))]"
                       id="DataHasCorrespondingNotice-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('DS')) and not ($elem/@ism:externalNotice=true()))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('DS')) and not ($elem/@ism:externalNotice=true()))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00134'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE, any
			element meeting ISM_CONTRIBUTES in the document has the attribute <xsl:text/>
                  <xsl:value-of select="'nonICmarkings'"/>
                  <xsl:text/> containing [<xsl:text/>
                  <xsl:value-of select="'DS'"/>
                  <xsl:text/>], then some
			element meeting ISM_CONTRIBUTES in the document MUST have attribute noticeType
			containing [<xsl:text/>
                  <xsl:value-of select="'DS'"/>
                  <xsl:text/>].</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M313"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M313"/>
   <xsl:template match="@*|node()" priority="-2" mode="M313">
      <xsl:apply-templates select="*" mode="M313"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00135-->


	<!--RULE NoticeHasCorrespondingData-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('RD'))]"
                 priority="1000"
                 mode="M314">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('RD'))]"
                       id="NoticeHasCorrespondingData-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of($partAtomicEnergyMarkings_tok, 'RD') &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of($partAtomicEnergyMarkings_tok, 'RD') &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
				[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00135'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE with no CUI, and any element
			meeting ISM_CONTRIBUTES in the document has the attribute noticeType containing
				[<xsl:text/>
                  <xsl:value-of select="'RD'"/>
                  <xsl:text/>], then some element meeting ISM_CONTRIBUTES in
			the document MUST have attribute <xsl:text/>
                  <xsl:value-of select="'atomicEnergyMarkings'"/>
                  <xsl:text/> containing
				[<xsl:text/>
                  <xsl:value-of select="'RD'"/>
                  <xsl:text/>]. Human Readable: USA documents containing an
				<xsl:text/>
                  <xsl:value-of select="'RD'"/>
                  <xsl:text/> notice must also have <xsl:text/>
                  <xsl:value-of select="'RD'"/>
                  <xsl:text/> data. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M314"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M314"/>
   <xsl:template match="@*|node()" priority="-2" mode="M314">
      <xsl:apply-templates select="*" mode="M314"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00136-->


	<!--RULE NoticeHasCorrespondingData-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('FRD'))]"
                 priority="1000"
                 mode="M315">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('FRD'))]"
                       id="NoticeHasCorrespondingData-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of($partAtomicEnergyMarkings_tok, 'FRD') &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of($partAtomicEnergyMarkings_tok, 'FRD') &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
				[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00136'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE with no CUI, and any element
			meeting ISM_CONTRIBUTES in the document has the attribute noticeType containing
				[<xsl:text/>
                  <xsl:value-of select="'FRD'"/>
                  <xsl:text/>], then some element meeting ISM_CONTRIBUTES in
			the document MUST have attribute <xsl:text/>
                  <xsl:value-of select="'atomicEnergyMarkings'"/>
                  <xsl:text/> containing
				[<xsl:text/>
                  <xsl:value-of select="'FRD'"/>
                  <xsl:text/>]. Human Readable: USA documents containing an
				<xsl:text/>
                  <xsl:value-of select="'FRD'"/>
                  <xsl:text/> notice must also have <xsl:text/>
                  <xsl:value-of select="'FRD'"/>
                  <xsl:text/> data. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M315"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M315"/>
   <xsl:template match="@*|node()" priority="-2" mode="M315">
      <xsl:apply-templates select="*" mode="M315"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00138-->


	<!--RULE NoticeHasCorrespondingDataUnclassDoc-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:classification, ('U'))   and not (@ism:externalNotice=true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('DS'))]"
                 priority="1000"
                 mode="M316">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:classification, ('U'))   and not (@ism:externalNotice=true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('DS'))]"
                       id="NoticeHasCorrespondingDataUnclassDoc-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of($partNonICmarkings_tok, 'DS')&gt;0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of($partNonICmarkings_tok, 'DS')&gt;0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00138'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE and any element meeting
			ISM_CONTRIBUTES in the document has the attribute noticeType containing [<xsl:text/>
                  <xsl:value-of select="'DS'"/>
                  <xsl:text/>], then some element meeting ISM_CONTRIBUTES in the document
			MUST have attribute <xsl:text/>
                  <xsl:value-of select="'nonICmarkings'"/>
                  <xsl:text/> containing [<xsl:text/>
                  <xsl:value-of select="'DS'"/>
                  <xsl:text/>]. Human Readable: USA documents containing an <xsl:text/>
                  <xsl:value-of select="'DS'"/>
                  <xsl:text/> notice must also have <xsl:text/>
                  <xsl:value-of select="'DS'"/>
                  <xsl:text/> data.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M316"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M316"/>
   <xsl:template match="@*|node()" priority="-2" mode="M316">
      <xsl:apply-templates select="*" mode="M316"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00139-->


	<!--RULE NoticeHasCorrespondingData-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('FISA'))]"
                 priority="1000"
                 mode="M317">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('FISA'))]"
                       id="NoticeHasCorrespondingData-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of(($partCuiSpecified_tok,$partDisseminationControls_tok), 'FISA') &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of(($partCuiSpecified_tok,$partDisseminationControls_tok), 'FISA') &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
				[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00139'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE with no CUI, and any element
			meeting ISM_CONTRIBUTES in the document has the attribute noticeType containing
				[<xsl:text/>
                  <xsl:value-of select="'FISA'"/>
                  <xsl:text/>], then some element meeting ISM_CONTRIBUTES in
			the document MUST have attribute <xsl:text/>
                  <xsl:value-of select="'disseminationControls or cuiSpecified'"/>
                  <xsl:text/> containing
				[<xsl:text/>
                  <xsl:value-of select="'FISA'"/>
                  <xsl:text/>]. Human Readable: USA documents containing an
				<xsl:text/>
                  <xsl:value-of select="'FISA'"/>
                  <xsl:text/> notice must also have <xsl:text/>
                  <xsl:value-of select="'FISA'"/>
                  <xsl:text/> data. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M317"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M317"/>
   <xsl:template match="@*|node()" priority="-2" mode="M317">
      <xsl:apply-templates select="*" mode="M317"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00143-->


	<!--RULE ISM-ID-00143-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and @ism:derivativelyClassifiedBy]"
                 priority="1000"
                 mode="M320">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and @ism:derivativelyClassifiedBy]"
                       id="ISM-ID-00143-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:derivedFrom"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:derivedFrom">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00143][Error] If ISM_USGOV_RESOURCE and attribute @ism:derivativelyClassifiedBy is specified, 
            then attribute @ism:derivedFrom must be specified. 
            
            Human Readable: Derivatively Classified data including DOE data requires
            a derived from value to be identified.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M320"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M320"/>
   <xsl:template match="@*|node()" priority="-2" mode="M320">
      <xsl:apply-templates select="*" mode="M320"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00148-->


	<!--RULE MutuallyExclusiveAttributeValues-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('LES', 'LES-NF'))]"
                 priority="1000"
                 mode="M324">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('LES', 'LES-NF'))]"
                       id="MutuallyExclusiveAttributeValues-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( for $token in tokenize(normalize-space(string(@ism:nonICmarkings)),' ') return  if($token = ('LES', 'LES-NF')) then 1 else null ) = 1"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( for $token in tokenize(normalize-space(string(@ism:nonICmarkings)),' ') return if($token = ('LES', 'LES-NF')) then 1 else null ) = 1">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			               <xsl:text/>
                  <xsl:value-of select="'   [ISM-ID-00148][Error] If ISM_USGOV_RESOURCE, then Name tokens    [LES] and [LES-NF] are mutually exclusive for attribute nonICmarkings.      Human Readable: USA documents must not specify both LES and LES-NF    on a single element.   '"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M324"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M324"/>
   <xsl:template match="@*|node()" priority="-2" mode="M324">
      <xsl:apply-templates select="*" mode="M324"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00150-->


	<!--RULE ISM-ID-00150-R1-->
<xsl:template match="*[($ISM_USGOV_RESOURCE or $ISM_USCUIONLY_RESOURCE) and not(generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)) and (util:containsAnyOfTheTokens(@ism:nonICmarkings, ('LES')) or util:containsAnyOfTheTokens(@ism:cuiBasic, ('LEI')))]"
                 priority="1000"
                 mode="M326">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USGOV_RESOURCE or $ISM_USCUIONLY_RESOURCE) and not(generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)) and (util:containsAnyOfTheTokens(@ism:nonICmarkings, ('LES')) or util:containsAnyOfTheTokens(@ism:cuiBasic, ('LEI')))]"
                       id="ISM-ID-00150-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('LES')) and not ($elem/@ism:externalNotice=true()))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('LES')) and not ($elem/@ism:externalNotice=true()))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
        [ISM-ID-00150][Error] If (ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE) and:
        1. Any element, other than ISM_RESOURCE_ELEMENT, meeting ISM_CONTRIBUTES in the document has the 
        attribute @ism:nonICmarkings containing [LES] or the attribute @ism:cuiBasic containing [LEI]
        AND
        2. No element meeting ISM_CONTRIBUTES in the document has the attribute @ism:noticeType containing [LES]
        
        Human Readable: USA documents containing LES non-IC markings or LEI cuiBasic markings must also have an 
        LES notice.
    </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M326"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M326"/>
   <xsl:template match="@*|node()" priority="-2" mode="M326">
      <xsl:apply-templates select="*" mode="M326"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00151-->


	<!--RULE NoticeHasCorrespondingDataTwoPossibleTokens-R1-->
<xsl:template match="*[($ISM_USGOV_RESOURCE or $ISM_USCUIONLY_RESOURCE) and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('LES'))]"
                 priority="1000"
                 mode="M327">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USGOV_RESOURCE or $ISM_USCUIONLY_RESOURCE) and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('LES'))]"
                       id="NoticeHasCorrespondingDataTwoPossibleTokens-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="(index-of(($partNonICmarkings_tok,$partCuiBasic_tok), 'LES') &gt; 0)    or (index-of(($partNonICmarkings_tok,$partCuiBasic_tok), 'LEI') &gt; 0)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="(index-of(($partNonICmarkings_tok,$partCuiBasic_tok), 'LES') &gt; 0) or (index-of(($partNonICmarkings_tok,$partCuiBasic_tok), 'LEI') &gt; 0)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00151'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE, and any element
			meeting ISM_CONTRIBUTES in the document has the attribute noticeType containing
				[<xsl:text/>
                  <xsl:value-of select="'LES'"/>
                  <xsl:text/>], then some element meeting ISM_CONTRIBUTES in
			the document MUST have attribute <xsl:text/>
                  <xsl:value-of select="'nonICmarkings or cuiBasic'"/>
                  <xsl:text/> containing
			[<xsl:text/>
                  <xsl:value-of select="'LES'"/>
                  <xsl:text/>] or [<xsl:text/>
                  <xsl:value-of select="'LEI'"/>
                  <xsl:text/>], respectively. Human Readable: USA documents containing an
				<xsl:text/>
                  <xsl:value-of select="'LES'"/>
                  <xsl:text/> notice must also have either [<xsl:text/>
                  <xsl:value-of select="'LES'"/>
                  <xsl:text/>] or [<xsl:text/>
                  <xsl:value-of select="'LEI'"/>
                  <xsl:text/>] data. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M327"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M327"/>
   <xsl:template match="@*|node()" priority="-2" mode="M327">
      <xsl:apply-templates select="*" mode="M327"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00152-->


	<!--RULE DataHasCorrespondingNotice-R1-->
<xsl:template match="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('LES-NF'))]"
                 priority="1000"
                 mode="M328">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('LES-NF'))]"
                       id="DataHasCorrespondingNotice-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('LES-NF')) and not ($elem/@ism:externalNotice=true()))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('LES-NF')) and not ($elem/@ism:externalNotice=true()))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00152'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE, any
			element meeting ISM_CONTRIBUTES in the document has the attribute <xsl:text/>
                  <xsl:value-of select="'nonICmarkings'"/>
                  <xsl:text/> containing [<xsl:text/>
                  <xsl:value-of select="'LES-NF'"/>
                  <xsl:text/>], then some
			element meeting ISM_CONTRIBUTES in the document MUST have attribute noticeType
			containing [<xsl:text/>
                  <xsl:value-of select="'LES-NF'"/>
                  <xsl:text/>].</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M328"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M328"/>
   <xsl:template match="@*|node()" priority="-2" mode="M328">
      <xsl:apply-templates select="*" mode="M328"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00153-->


	<!--RULE NoticeHasCorrespondingData-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('LES-NF'))]"
                 priority="1000"
                 mode="M329">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('LES-NF'))]"
                       id="NoticeHasCorrespondingData-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of($partNonICmarkings_tok, 'LES-NF') &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of($partNonICmarkings_tok, 'LES-NF') &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
				[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00153'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE with no CUI, and any element
			meeting ISM_CONTRIBUTES in the document has the attribute noticeType containing
				[<xsl:text/>
                  <xsl:value-of select="'LES-NF'"/>
                  <xsl:text/>], then some element meeting ISM_CONTRIBUTES in
			the document MUST have attribute <xsl:text/>
                  <xsl:value-of select="'nonICmarkings'"/>
                  <xsl:text/> containing
				[<xsl:text/>
                  <xsl:value-of select="'LES-NF'"/>
                  <xsl:text/>]. Human Readable: USA documents containing an
				<xsl:text/>
                  <xsl:value-of select="'LES-NF'"/>
                  <xsl:text/> notice must also have <xsl:text/>
                  <xsl:value-of select="'LES-NF'"/>
                  <xsl:text/> data. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M329"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M329"/>
   <xsl:template match="@*|node()" priority="-2" mode="M329">
      <xsl:apply-templates select="*" mode="M329"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00159-->


	<!--RULE ISM-ID-00159-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and not($ISM_RESOURCE_ELEMENT/@ism:classification = 'U')]"
                 priority="1000"
                 mode="M331">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and not($ISM_RESOURCE_ELEMENT/@ism:classification = 'U')]"
                       id="ISM-ID-00159-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(util:containsAnyOfTheTokens(@ism:noticeType, ('DoD-Dist-A'))) or (@ism:externalNotice=true())"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(util:containsAnyOfTheTokens(@ism:noticeType, ('DoD-Dist-A'))) or (@ism:externalNotice=true())">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> 
            [ISM-ID-00159][Error] If ISM_USGOV_RESOURCE and:
            1. attribute @ism:classification of ISM_RESOURCE_ELEMENT is not [U]
            AND
            2. The attribute @ism:noticeType does contain [DoD-Dist-A] or has attribute @ism:externalNotice with a value of [true].
            
            Human Readable: Distribution statement A (Public Release) is forbidden on classified documents.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M331"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M331"/>
   <xsl:template match="@*|node()" priority="-2" mode="M331">
      <xsl:apply-templates select="*" mode="M331"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00164-->


	<!--RULE ISM-ID-00164-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('RS'))]"
                 priority="1000"
                 mode="M332">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('RS'))]"
                       id="ISM-ID-00164-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:classification=('TS', 'S')"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="@ism:classification=('TS', 'S')">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00164][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls contains the name token [RS],
            then attribute @ism:classification must have a value of [TS] or [S].
            
            Human Readable: USA documents with RISK SENSITIVE dissemination must
            be classified SECRET or TOP SECRET.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M332"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M332"/>
   <xsl:template match="@*|node()" priority="-2" mode="M332">
      <xsl:apply-templates select="*" mode="M332"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00166-->


	<!--RULE AttributeValueDeprecatedWarning-R1-->
<xsl:template match="*[@ism:classification]" priority="1000" mode="M334">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:classification]"
                       id="AttributeValueDeprecatedWarning-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:classification), document('../../CVE/ISM/CVEnumISMClassificationAll.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:classification), document('../../CVE/ISM/CVEnumISMClassificationAll.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00166'"/>
                  <xsl:text/>][Warning] For attribute <xsl:text/>
                  <xsl:value-of select="'classification'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated(string(@ism:classification), document('../../CVE/ISM/CVEnumISMClassificationAll.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE,false())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M334"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M334"/>
   <xsl:template match="@*|node()" priority="-2" mode="M334">
      <xsl:apply-templates select="*" mode="M334"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00168-->


	<!--RULE ISM-ID-00168-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('DISPLAYONLY')))]"
                 priority="1000"
                 mode="M335">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('DISPLAYONLY')))]"
                       id="ISM-ID-00168-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:displayOnlyTo)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="not(@ism:displayOnlyTo)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00168][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls is not specified or is specified and does not contain the name token 
            [DISPLAYONLY], then attribute @ism:displayOnlyTo must not be specified.
            
            Human Readable: If a portion in a USA document is not marked for DISPLAY ONLY dissemination, 
            it must not list countries to which it may be disclosed. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M335"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M335"/>
   <xsl:template match="@*|node()" priority="-2" mode="M335">
      <xsl:apply-templates select="*" mode="M335"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00169-->


	<!--RULE MutuallyExclusiveAttributeValues-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('DISPLAYONLY', 'RELIDO', 'NF'))]"
                 priority="1000"
                 mode="M336">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('DISPLAYONLY', 'RELIDO', 'NF'))]"
                       id="MutuallyExclusiveAttributeValues-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( for $token in tokenize(normalize-space(string(@ism:disseminationControls)),' ') return  if($token = ('DISPLAYONLY', 'RELIDO', 'NF')) then 1 else null ) = 1"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( for $token in tokenize(normalize-space(string(@ism:disseminationControls)),' ') return if($token = ('DISPLAYONLY', 'RELIDO', 'NF')) then 1 else null ) = 1">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			               <xsl:text/>
                  <xsl:value-of select="'    [ISM-ID-00169][Error] If ISM_USGOV_RESOURCE, then for attribute disseminationControls     the name tokens [DISPLAYONLY], [RELIDO] and [NF] are mutually exclusive.        Human Readable: In a USA document, DISPLAY ONLY, RELIDO and NO FOREIGN dissemination are     mutually exclusive for a single element.   '"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M336"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M336"/>
   <xsl:template match="@*|node()" priority="-2" mode="M336">
      <xsl:apply-templates select="*" mode="M336"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00170-->


	<!--RULE AttributeValueDeprecatedError-R1-->
<xsl:template match="*[@ism:classification]" priority="1000" mode="M337">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:classification]"
                       id="AttributeValueDeprecatedError-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:classification), document('../../CVE/ISM/CVEnumISMClassificationAll.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:classification), document('../../CVE/ISM/CVEnumISMClassificationAll.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00170'"/>
                  <xsl:text/>][Error] For attribute <xsl:text/>
                  <xsl:value-of select="'classification'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated( string(@ism:classification), document('../../CVE/ISM/CVEnumISMClassificationAll.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE, true())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M337"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M337"/>
   <xsl:template match="@*|node()" priority="-2" mode="M337">
      <xsl:apply-templates select="*" mode="M337"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00173-->


	<!--RULE ISM-ID-00173-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:atomicEnergyMarkings, ('^RD-SG', '^FRD-SG'))]"
                 priority="1000"
                 mode="M338">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:atomicEnergyMarkings, ('^RD-SG', '^FRD-SG'))]"
                       id="ISM-ID-00173-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:classification = ('S','TS')"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="@ism:classification = ('S','TS')">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		        [ISM-ID-00173][Error] If ISM_USGOV_RESOURCE and attribute
		        @ism:atomicEnergyMarkings contains a name token starting with [RD-SG] or [FRD-SG], then attribute
		        @ism:classification must have a value of [S] or [TS]. 
		        
		        Human Readable: Portions in a USA document that contain RD or FRD SIGMA data must be marked SECRET or TOP SECRET. 
		    </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M338"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M338"/>
   <xsl:template match="@*|node()" priority="-2" mode="M338">
      <xsl:apply-templates select="*" mode="M338"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00174-->


	<!--RULE ISM-ID-00174-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD', 'FRD', 'TFNI'))]"
                 priority="1000"
                 mode="M339">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD', 'FRD', 'TFNI'))]"
                       id="ISM-ID-00174-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:classification = ('TS','S','C')"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="@ism:classification = ('TS','S','C')">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		        [ISM-ID-00174][Error] If ISM_USGOV_RESOURCE and attribute 
		        @ism:atomicEnergyMarkings contains the name token [RD], [FRD], or [TFNI], 
		        then attribute @ism:classification must have a value of [TS], [S], or [C].
		        
		        Human Readable: USA documents with RD, FRD, or TFNI data must be marked CONFIDENTIAL,
		        SECRET, or TOP SECRET.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M339"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M339"/>
   <xsl:template match="@*|node()" priority="-2" mode="M339">
      <xsl:apply-templates select="*" mode="M339"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00175-->


	<!--RULE ISM-ID-00175-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD-CNWDI'))]"
                 priority="1000"
                 mode="M340">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD-CNWDI'))]"
                       id="ISM-ID-00175-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:classification = ('TS','S')"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="@ism:classification = ('TS','S')">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00175][Error] If ISM_USGOV_RESOURCE and attribute 
		    	@ism:atomicEnergyMarkings contains the name token [RD-CNWDI], then attribute 
		    	@ism:classification must have a value of [TS] or [S].
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M340"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M340"/>
   <xsl:template match="@*|node()" priority="-2" mode="M340">
      <xsl:apply-templates select="*" mode="M340"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00179-->


	<!--RULE AttributeValueDeprecatedWarning-R1-->
<xsl:template match="*[@ism:disseminationControls]" priority="1000" mode="M342">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:disseminationControls]"
                       id="AttributeValueDeprecatedWarning-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:disseminationControls), document('../../CVE/ISM/CVEnumISMDissem.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:disseminationControls), document('../../CVE/ISM/CVEnumISMDissem.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00179'"/>
                  <xsl:text/>][Warning] For attribute <xsl:text/>
                  <xsl:value-of select="'disseminationControls'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated(string(@ism:disseminationControls), document('../../CVE/ISM/CVEnumISMDissem.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE,false())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M342"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M342"/>
   <xsl:template match="@*|node()" priority="-2" mode="M342">
      <xsl:apply-templates select="*" mode="M342"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00180-->


	<!--RULE AttributeValueDeprecatedError-R1-->
<xsl:template match="*[@ism:disseminationControls]" priority="1000" mode="M343">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:disseminationControls]"
                       id="AttributeValueDeprecatedError-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:disseminationControls), document('../../CVE/ISM/CVEnumISMDissem.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:disseminationControls), document('../../CVE/ISM/CVEnumISMDissem.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00180'"/>
                  <xsl:text/>][Error] For attribute <xsl:text/>
                  <xsl:value-of select="'disseminationControls'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated( string(@ism:disseminationControls), document('../../CVE/ISM/CVEnumISMDissem.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE, true())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M343"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M343"/>
   <xsl:template match="@*|node()" priority="-2" mode="M343">
      <xsl:apply-templates select="*" mode="M343"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00181-->


	<!--RULE ISM-ID-00181-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and @ism:atomicEnergyMarkings and not(@ism:classification='U')]"
                 priority="1000"
                 mode="M344">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and @ism:atomicEnergyMarkings and not(@ism:classification='U')]"
                       id="ISM-ID-00181-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('UCNI', 'DCNI')))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('UCNI', 'DCNI')))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		        [ISM-ID-00181][Error] If ISM_USGOV_RESOURCE and element's classification does not have a value of "U" 
		        then attribute @ism:atomicEnergyMarkings must not contain the name token [UCNI] or [DCNI].
		        
		        Human Readable: UCNI and DCNI may only be used on UNCLASSIFIED portions.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M344"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M344"/>
   <xsl:template match="@*|node()" priority="-2" mode="M344">
      <xsl:apply-templates select="*" mode="M344"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00183-->


	<!--RULE ISM-ID-00183-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:atomicEnergyMarkings, ('^RD-SG'))]"
                 priority="1000"
                 mode="M345">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:atomicEnergyMarkings, ('^RD-SG'))]"
                       id="ISM-ID-00183-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00183][Error] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings 
		    	contains a name token starting with [RD-SG], then it must also contain the name token [RD].
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M345"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M345"/>
   <xsl:template match="@*|node()" priority="-2" mode="M345">
      <xsl:apply-templates select="*" mode="M345"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00184-->


	<!--RULE ISM-ID-00184-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:atomicEnergyMarkings, ('^FRD-SG'))]"
                 priority="1000"
                 mode="M346">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:atomicEnergyMarkings, ('^FRD-SG'))]"
                       id="ISM-ID-00184-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('FRD'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('FRD'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00184][Error] If ISM_USGOV_RESOURCE and attribute 
		    	@ism:atomicEnergyMarkings contains a name token starting with [FRD-SG],
		    	then it must also contain the name token [FRD].
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M346"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M346"/>
   <xsl:template match="@*|node()" priority="-2" mode="M346">
      <xsl:apply-templates select="*" mode="M346"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00185-->


	<!--RULE ISM-ID-00185-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:atomicEnergyMarkings, ('RD-CNWDI'))]"
                 priority="1000"
                 mode="M347">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:atomicEnergyMarkings, ('RD-CNWDI'))]"
                       id="ISM-ID-00185-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00185][Error] If ISM_USGOV_RESOURCE and attribute 
		    	@ism:atomicEnergyMarkings contains the name token [RD-CNWDI],
		    	then it must also contain the name token [RD].
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M347"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M347"/>
   <xsl:template match="@*|node()" priority="-2" mode="M347">
      <xsl:apply-templates select="*" mode="M347"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00188-->


	<!--RULE AttributeValueDeprecatedWarning-R1-->
<xsl:template match="*[@ism:FGIsourceOpen]" priority="1000" mode="M348">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:FGIsourceOpen]"
                       id="AttributeValueDeprecatedWarning-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:FGIsourceOpen), document('../../CVE/ISMCAT/CVEnumISMCATFGIOpen.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:FGIsourceOpen), document('../../CVE/ISMCAT/CVEnumISMCATFGIOpen.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00188'"/>
                  <xsl:text/>][Warning] For attribute <xsl:text/>
                  <xsl:value-of select="'FGIsourceOpen'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated(string(@ism:FGIsourceOpen), document('../../CVE/ISMCAT/CVEnumISMCATFGIOpen.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE,false())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M348"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M348"/>
   <xsl:template match="@*|node()" priority="-2" mode="M348">
      <xsl:apply-templates select="*" mode="M348"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00189-->


	<!--RULE AttributeValueDeprecatedError-R1-->
<xsl:template match="*[@ism:FGIsourceOpen]" priority="1000" mode="M349">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:FGIsourceOpen]"
                       id="AttributeValueDeprecatedError-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:FGIsourceOpen), document('../../CVE/ISMCAT/CVEnumISMCATFGIOpen.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:FGIsourceOpen), document('../../CVE/ISMCAT/CVEnumISMCATFGIOpen.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00189'"/>
                  <xsl:text/>][Error] For attribute <xsl:text/>
                  <xsl:value-of select="'FGIsourceOpen'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated( string(@ism:FGIsourceOpen), document('../../CVE/ISMCAT/CVEnumISMCATFGIOpen.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE, true())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M349"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M349"/>
   <xsl:template match="@*|node()" priority="-2" mode="M349">
      <xsl:apply-templates select="*" mode="M349"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00190-->


	<!--RULE AttributeValueDeprecatedWarning-R1-->
<xsl:template match="*[@ism:FGIsourceProtected]" priority="1000" mode="M350">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:FGIsourceProtected]"
                       id="AttributeValueDeprecatedWarning-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:FGIsourceProtected), document('../../CVE/ISMCAT/CVEnumISMCATFGIProtected.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:FGIsourceProtected), document('../../CVE/ISMCAT/CVEnumISMCATFGIProtected.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00190'"/>
                  <xsl:text/>][Warning] For attribute <xsl:text/>
                  <xsl:value-of select="'FGIsourceProtected'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated(string(@ism:FGIsourceProtected), document('../../CVE/ISMCAT/CVEnumISMCATFGIProtected.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE,false())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M350"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M350"/>
   <xsl:template match="@*|node()" priority="-2" mode="M350">
      <xsl:apply-templates select="*" mode="M350"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00191-->


	<!--RULE AttributeValueDeprecatedError-R1-->
<xsl:template match="*[@ism:FGIsourceProtected]" priority="1000" mode="M351">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:FGIsourceProtected]"
                       id="AttributeValueDeprecatedError-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:FGIsourceProtected), document('../../CVE/ISMCAT/CVEnumISMCATFGIProtected.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:FGIsourceProtected), document('../../CVE/ISMCAT/CVEnumISMCATFGIProtected.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00191'"/>
                  <xsl:text/>][Error] For attribute <xsl:text/>
                  <xsl:value-of select="'FGIsourceProtected'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated( string(@ism:FGIsourceProtected), document('../../CVE/ISMCAT/CVEnumISMCATFGIProtected.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE, true())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M351"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M351"/>
   <xsl:template match="@*|node()" priority="-2" mode="M351">
      <xsl:apply-templates select="*" mode="M351"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00192-->


	<!--RULE AttributeValueDeprecatedWarning-R1-->
<xsl:template match="*[@ism:nonICmarkings]" priority="1000" mode="M352">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:nonICmarkings]"
                       id="AttributeValueDeprecatedWarning-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:nonICmarkings), document('../../CVE/ISM/CVEnumISMNonIC.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:nonICmarkings), document('../../CVE/ISM/CVEnumISMNonIC.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00192'"/>
                  <xsl:text/>][Warning] For attribute <xsl:text/>
                  <xsl:value-of select="'nonICmarkings'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated(string(@ism:nonICmarkings), document('../../CVE/ISM/CVEnumISMNonIC.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE,false())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M352"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M352"/>
   <xsl:template match="@*|node()" priority="-2" mode="M352">
      <xsl:apply-templates select="*" mode="M352"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00193-->


	<!--RULE AttributeValueDeprecatedError-R1-->
<xsl:template match="*[@ism:nonICmarkings]" priority="1000" mode="M353">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:nonICmarkings]"
                       id="AttributeValueDeprecatedError-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:nonICmarkings), document('../../CVE/ISM/CVEnumISMNonIC.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:nonICmarkings), document('../../CVE/ISM/CVEnumISMNonIC.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00193'"/>
                  <xsl:text/>][Error] For attribute <xsl:text/>
                  <xsl:value-of select="'nonICmarkings'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated( string(@ism:nonICmarkings), document('../../CVE/ISM/CVEnumISMNonIC.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE, true())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M353"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M353"/>
   <xsl:template match="@*|node()" priority="-2" mode="M353">
      <xsl:apply-templates select="*" mode="M353"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00196-->


	<!--RULE AttributeValueDeprecatedWarning-R1-->
<xsl:template match="*[@ism:ownerProducer]" priority="1000" mode="M354">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:ownerProducer]"
                       id="AttributeValueDeprecatedWarning-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:ownerProducer), document('../../CVE/ISMCAT/CVEnumISMCATOwnerProducer.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:ownerProducer), document('../../CVE/ISMCAT/CVEnumISMCATOwnerProducer.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00196'"/>
                  <xsl:text/>][Warning] For attribute <xsl:text/>
                  <xsl:value-of select="'ownerProducer'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated(string(@ism:ownerProducer), document('../../CVE/ISMCAT/CVEnumISMCATOwnerProducer.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE,false())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M354"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M354"/>
   <xsl:template match="@*|node()" priority="-2" mode="M354">
      <xsl:apply-templates select="*" mode="M354"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00197-->


	<!--RULE AttributeValueDeprecatedError-R1-->
<xsl:template match="*[@ism:ownerProducer]" priority="1000" mode="M355">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:ownerProducer]"
                       id="AttributeValueDeprecatedError-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:ownerProducer), document('../../CVE/ISMCAT/CVEnumISMCATOwnerProducer.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:ownerProducer), document('../../CVE/ISMCAT/CVEnumISMCATOwnerProducer.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00197'"/>
                  <xsl:text/>][Error] For attribute <xsl:text/>
                  <xsl:value-of select="'ownerProducer'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated( string(@ism:ownerProducer), document('../../CVE/ISMCAT/CVEnumISMCATOwnerProducer.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE, true())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M355"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M355"/>
   <xsl:template match="@*|node()" priority="-2" mode="M355">
      <xsl:apply-templates select="*" mode="M355"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00198-->


	<!--RULE AttributeValueDeprecatedWarning-R1-->
<xsl:template match="*[@ism:releasableTo]" priority="1000" mode="M356">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:releasableTo]"
                       id="AttributeValueDeprecatedWarning-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:releasableTo), document('../../CVE/ISMCAT/CVEnumISMCATRelTo.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:releasableTo), document('../../CVE/ISMCAT/CVEnumISMCATRelTo.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00198'"/>
                  <xsl:text/>][Warning] For attribute <xsl:text/>
                  <xsl:value-of select="'releasableTo'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated(string(@ism:releasableTo), document('../../CVE/ISMCAT/CVEnumISMCATRelTo.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE,false())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M356"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M356"/>
   <xsl:template match="@*|node()" priority="-2" mode="M356">
      <xsl:apply-templates select="*" mode="M356"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00199-->


	<!--RULE AttributeValueDeprecatedError-R1-->
<xsl:template match="*[@ism:releasableTo]" priority="1000" mode="M357">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:releasableTo]"
                       id="AttributeValueDeprecatedError-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:releasableTo), document('../../CVE/ISMCAT/CVEnumISMCATRelTo.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:releasableTo), document('../../CVE/ISMCAT/CVEnumISMCATRelTo.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00199'"/>
                  <xsl:text/>][Error] For attribute <xsl:text/>
                  <xsl:value-of select="'releasableTo'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated( string(@ism:releasableTo), document('../../CVE/ISMCAT/CVEnumISMCATRelTo.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE, true())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M357"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M357"/>
   <xsl:template match="@*|node()" priority="-2" mode="M357">
      <xsl:apply-templates select="*" mode="M357"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00200-->


	<!--RULE AttributeValueDeprecatedWarning-R1-->
<xsl:template match="*[@ism:displayOnlyTo]" priority="1000" mode="M358">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:displayOnlyTo]"
                       id="AttributeValueDeprecatedWarning-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:displayOnlyTo), document('../../CVE/ISMCAT/CVEnumISMCATRelTo.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:displayOnlyTo), document('../../CVE/ISMCAT/CVEnumISMCATRelTo.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00200'"/>
                  <xsl:text/>][Warning] For attribute <xsl:text/>
                  <xsl:value-of select="'displayOnlyTo'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated(string(@ism:displayOnlyTo), document('../../CVE/ISMCAT/CVEnumISMCATRelTo.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE,false())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M358"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M358"/>
   <xsl:template match="@*|node()" priority="-2" mode="M358">
      <xsl:apply-templates select="*" mode="M358"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00201-->


	<!--RULE AttributeValueDeprecatedError-R1-->
<xsl:template match="*[@ism:displayOnlyTo]" priority="1000" mode="M359">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:displayOnlyTo]"
                       id="AttributeValueDeprecatedError-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:displayOnlyTo), document('../../CVE/ISMCAT/CVEnumISMCATRelTo.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:displayOnlyTo), document('../../CVE/ISMCAT/CVEnumISMCATRelTo.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00201'"/>
                  <xsl:text/>][Error] For attribute <xsl:text/>
                  <xsl:value-of select="'displayOnlyTo'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated( string(@ism:displayOnlyTo), document('../../CVE/ISMCAT/CVEnumISMCATRelTo.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE, true())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M359"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M359"/>
   <xsl:template match="@*|node()" priority="-2" mode="M359">
      <xsl:apply-templates select="*" mode="M359"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00202-->


	<!--RULE AttributeValueDeprecatedWarning-R1-->
<xsl:template match="*[@ism:SARIdentifier]" priority="1000" mode="M360">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:SARIdentifier]"
                       id="AttributeValueDeprecatedWarning-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:SARIdentifier), document('../../CVE/ISM/CVEnumISMSAR.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:SARIdentifier), document('../../CVE/ISM/CVEnumISMSAR.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00202'"/>
                  <xsl:text/>][Warning] For attribute <xsl:text/>
                  <xsl:value-of select="'SARIdentifier'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated(string(@ism:SARIdentifier), document('../../CVE/ISM/CVEnumISMSAR.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE,false())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M360"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M360"/>
   <xsl:template match="@*|node()" priority="-2" mode="M360">
      <xsl:apply-templates select="*" mode="M360"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00203-->


	<!--RULE AttributeValueDeprecatedError-R1-->
<xsl:template match="*[@ism:SARIdentifier]" priority="1000" mode="M361">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:SARIdentifier]"
                       id="AttributeValueDeprecatedError-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:SARIdentifier), document('../../CVE/ISM/CVEnumISMSAR.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:SARIdentifier), document('../../CVE/ISM/CVEnumISMSAR.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00203'"/>
                  <xsl:text/>][Error] For attribute <xsl:text/>
                  <xsl:value-of select="'SARIdentifier'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated( string(@ism:SARIdentifier), document('../../CVE/ISM/CVEnumISMSAR.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE, true())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M361"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M361"/>
   <xsl:template match="@*|node()" priority="-2" mode="M361">
      <xsl:apply-templates select="*" mode="M361"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00204-->


	<!--RULE AttributeValueDeprecatedWarning-R1-->
<xsl:template match="*[@ism:SCIcontrols]" priority="1000" mode="M362">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:SCIcontrols]"
                       id="AttributeValueDeprecatedWarning-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:SCIcontrols), document('../../CVE/ISM/CVEnumISMSCIControls.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:SCIcontrols), document('../../CVE/ISM/CVEnumISMSCIControls.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00204'"/>
                  <xsl:text/>][Warning] For attribute <xsl:text/>
                  <xsl:value-of select="'SCIcontrols'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated(string(@ism:SCIcontrols), document('../../CVE/ISM/CVEnumISMSCIControls.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE,false())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M362"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M362"/>
   <xsl:template match="@*|node()" priority="-2" mode="M362">
      <xsl:apply-templates select="*" mode="M362"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00205-->


	<!--RULE AttributeValueDeprecatedError-R1-->
<xsl:template match="*[@ism:SCIcontrols]" priority="1000" mode="M363">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:SCIcontrols]"
                       id="AttributeValueDeprecatedError-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:SCIcontrols), document('../../CVE/ISM/CVEnumISMSCIControls.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:SCIcontrols), document('../../CVE/ISM/CVEnumISMSCIControls.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00205'"/>
                  <xsl:text/>][Error] For attribute <xsl:text/>
                  <xsl:value-of select="'SCIcontrols'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated( string(@ism:SCIcontrols), document('../../CVE/ISM/CVEnumISMSCIControls.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE, true())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M363"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M363"/>
   <xsl:template match="@*|node()" priority="-2" mode="M363">
      <xsl:apply-templates select="*" mode="M363"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00206-->


	<!--RULE AttributeValueDeprecatedWarning-R1-->
<xsl:template match="*[@ism:declassException]" priority="1000" mode="M364">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:declassException]"
                       id="AttributeValueDeprecatedWarning-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:declassException), document('../../CVE/ISM/CVEnumISM25X.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:declassException), document('../../CVE/ISM/CVEnumISM25X.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00206'"/>
                  <xsl:text/>][Warning] For attribute <xsl:text/>
                  <xsl:value-of select="'declassException'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated(string(@ism:declassException), document('../../CVE/ISM/CVEnumISM25X.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE,false())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M364"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M364"/>
   <xsl:template match="@*|node()" priority="-2" mode="M364">
      <xsl:apply-templates select="*" mode="M364"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00207-->


	<!--RULE AttributeValueDeprecatedError-R1-->
<xsl:template match="*[@ism:declassException]" priority="1000" mode="M365">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:declassException]"
                       id="AttributeValueDeprecatedError-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:declassException), document('../../CVE/ISM/CVEnumISM25X.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:declassException), document('../../CVE/ISM/CVEnumISM25X.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00207'"/>
                  <xsl:text/>][Error] For attribute <xsl:text/>
                  <xsl:value-of select="'declassException'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated( string(@ism:declassException), document('../../CVE/ISM/CVEnumISM25X.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE, true())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M365"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M365"/>
   <xsl:template match="@*|node()" priority="-2" mode="M365">
      <xsl:apply-templates select="*" mode="M365"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00208-->


	<!--RULE AttributeValueDeprecatedWarning-R1-->
<xsl:template match="*[@ism:atomicEnergyMarkings]" priority="1000" mode="M366">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:atomicEnergyMarkings]"
                       id="AttributeValueDeprecatedWarning-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:atomicEnergyMarkings), document('../../CVE/ISM/CVEnumISMAtomicEnergyMarkings.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:atomicEnergyMarkings), document('../../CVE/ISM/CVEnumISMAtomicEnergyMarkings.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00208'"/>
                  <xsl:text/>][Warning] For attribute <xsl:text/>
                  <xsl:value-of select="'atomicEnergyMarkings'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated(string(@ism:atomicEnergyMarkings), document('../../CVE/ISM/CVEnumISMAtomicEnergyMarkings.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE,false())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M366"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M366"/>
   <xsl:template match="@*|node()" priority="-2" mode="M366">
      <xsl:apply-templates select="*" mode="M366"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00209-->


	<!--RULE AttributeValueDeprecatedError-R1-->
<xsl:template match="*[@ism:atomicEnergyMarkings]" priority="1000" mode="M367">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:atomicEnergyMarkings]"
                       id="AttributeValueDeprecatedError-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:atomicEnergyMarkings), document('../../CVE/ISM/CVEnumISMAtomicEnergyMarkings.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:atomicEnergyMarkings), document('../../CVE/ISM/CVEnumISMAtomicEnergyMarkings.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00209'"/>
                  <xsl:text/>][Error] For attribute <xsl:text/>
                  <xsl:value-of select="'atomicEnergyMarkings'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated( string(@ism:atomicEnergyMarkings), document('../../CVE/ISM/CVEnumISMAtomicEnergyMarkings.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE, true())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M367"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M367"/>
   <xsl:template match="@*|node()" priority="-2" mode="M367">
      <xsl:apply-templates select="*" mode="M367"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00210-->


	<!--RULE AttributeValueDeprecatedWarning-R1-->
<xsl:template match="*[@ism:nonUSControls]" priority="1000" mode="M368">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:nonUSControls]"
                       id="AttributeValueDeprecatedWarning-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:nonUSControls), document('../../CVE/ISM/CVEnumISMNonUSControls.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:nonUSControls), document('../../CVE/ISM/CVEnumISMNonUSControls.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00210'"/>
                  <xsl:text/>][Warning] For attribute <xsl:text/>
                  <xsl:value-of select="'nonUSControls'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated(string(@ism:nonUSControls), document('../../CVE/ISM/CVEnumISMNonUSControls.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE,false())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M368"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M368"/>
   <xsl:template match="@*|node()" priority="-2" mode="M368">
      <xsl:apply-templates select="*" mode="M368"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00211-->


	<!--RULE AttributeValueDeprecatedError-R1-->
<xsl:template match="*[@ism:nonUSControls]" priority="1000" mode="M369">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:nonUSControls]"
                       id="AttributeValueDeprecatedError-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:nonUSControls), document('../../CVE/ISM/CVEnumISMNonUSControls.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:nonUSControls), document('../../CVE/ISM/CVEnumISMNonUSControls.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00211'"/>
                  <xsl:text/>][Error] For attribute <xsl:text/>
                  <xsl:value-of select="'nonUSControls'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated( string(@ism:nonUSControls), document('../../CVE/ISM/CVEnumISMNonUSControls.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE, true())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M369"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M369"/>
   <xsl:template match="@*|node()" priority="-2" mode="M369">
      <xsl:apply-templates select="*" mode="M369"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00213-->


	<!--RULE ISM-ID-00213-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('DISPLAYONLY'))]"
                 priority="1000"
                 mode="M370">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('DISPLAYONLY'))]"
                       id="ISM-ID-00213-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:displayOnlyTo"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:displayOnlyTo">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00213][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls contains the name token [DISPLAYONLY], then 
            attribute @ism:displayOnlyTo must be specified.
            
            Human Readable: A USA document with DISPLAY ONLY dissemination must 
            indicate the countries to which it may be disclosed.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M370"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M370"/>
   <xsl:template match="@*|node()" priority="-2" mode="M370">
      <xsl:apply-templates select="*" mode="M370"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00214-->


	<!--RULE ISM-ID-00214-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and @ism:releasableTo]"
                 priority="1000"
                 mode="M371">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and @ism:releasableTo]"
                       id="ISM-ID-00214-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of(tokenize(normalize-space(string(@ism:releasableTo)),' '),'USA')=1"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of(tokenize(normalize-space(string(@ism:releasableTo)),' '),'USA')=1">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00214][Error] If ISM_USGOV_RESOURCE then attribute @ism:releasableTo must start with [USA].
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M371"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M371"/>
   <xsl:template match="@*|node()" priority="-2" mode="M371">
      <xsl:apply-templates select="*" mode="M371"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00217-->


	<!--RULE ISM-ID-00217-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and @ism:FGIsourceProtected]"
                 priority="1000"
                 mode="M372">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and @ism:FGIsourceProtected]"
                       id="ISM-ID-00217-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="normalize-space(string(@ism:FGIsourceProtected))='FGI'"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="normalize-space(string(@ism:FGIsourceProtected))='FGI'">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		        [ISM-ID-00217][Error] If ISM_USGOV_RESOURCE attribute @ism:FGIsourceProtected contains [FGI], it must be the only value.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M372"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M372"/>
   <xsl:template match="@*|node()" priority="-2" mode="M372">
      <xsl:apply-templates select="*" mode="M372"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00221-->


	<!--RULE ISM-ID-00221-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and @ism:derivativelyClassifiedBy]"
                 priority="1000"
                 mode="M374">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and @ism:derivativelyClassifiedBy]"
                       id="ISM-ID-00221-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:classificationReason or @ism:classifiedBy)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(@ism:classificationReason or @ism:classifiedBy)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
	          [ISM-ID-00221][Error] If ISM_USGOV_RESOURCE and attribute 
	          @ism:derivativelyClassifiedBy is specified, then attributes @ism:classificationReason
	          or @ism:classifiedBy must not be specified.
	          
	          Human Readable: USA documents that are derivatively classified must not
	          specify a classification reason or classified by.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M374"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M374"/>
   <xsl:template match="@*|node()" priority="-2" mode="M374">
      <xsl:apply-templates select="*" mode="M374"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00223-->


	<!--RULE ValidateValueExistenceInList-R1-->
<xsl:template match="ism:*" priority="1000" mode="M375">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ism:*"
                       id="ValidateValueExistenceInList-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $token in $validElementList satisfies $token = local-name()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $token in $validElementList satisfies $token = local-name()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'   [ISM-ID-00223][Error] If any elements in namespace    urn:us:gov:ic:ism exist, the local name must exist in CVEnumISMElements.xml.       Human Readable: Ensure that elements in the ISM namespace are defined by ISM.XML.   '"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M375"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M375"/>
   <xsl:template match="@*|node()" priority="-2" mode="M375">
      <xsl:apply-templates select="*" mode="M375"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00226-->


	<!--RULE ISM-ID-00226-R1-->
<xsl:template match="*[@ism:noticeType]" priority="1000" mode="M376">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:noticeType]"
                       id="ISM-ID-00226-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:unregisteredNoticeType)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(@ism:unregisteredNoticeType)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00226][Error] Attributes @ism:noticeType and @ism:unregisteredNoticeType
            may not both be used on the same element. 
            
            Human Readable: Ensure that the ISM attributes noticeType and
            unregisteredNoticeType are not used on the same element.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M376"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M376"/>
   <xsl:template match="@*|node()" priority="-2" mode="M376">
      <xsl:apply-templates select="*" mode="M376"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00242-->


	<!--RULE ISM-ID-00242-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('RSV'))]"
                 priority="1000"
                 mode="M381">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('RSV'))]"
                       id="ISM-ID-00242-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00242][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [RSV],
            then it must also have attribute @ism:classification with a value of [S] or [TS].
            
            Human Readable: A USA document that contains RESERVE data must be classified SECRET or TOP SECRET.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M381"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M381"/>
   <xsl:template match="@*|node()" priority="-2" mode="M381">
      <xsl:apply-templates select="*" mode="M381"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00243-->


	<!--RULE ISM-ID-00243-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('RSV'))]"
                 priority="1000"
                 mode="M382">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('RSV'))]"
                       id="ISM-ID-00243-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyTokenMatching(@ism:SCIcontrols, ('RSV-[A-Z0-9]{3}'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyTokenMatching(@ism:SCIcontrols, ('RSV-[A-Z0-9]{3}'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
        [ISM-ID-00243][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [RSV],
        then it must also contain a compartment [RSV-XXX].
        
        Human Readable: RESERVE is not permitted as a stand-alone value and a compartment must be expressed.
    </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M382"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M382"/>
   <xsl:template match="@*|node()" priority="-2" mode="M382">
      <xsl:apply-templates select="*" mode="M382"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00244-->


	<!--RULE ISM-ID-00244-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD-CNWDI'))]"
                 priority="1000"
                 mode="M383">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD-CNWDI'))]"
                       id="ISM-ID-00244-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('CNWDI')) and not ($elem/@ism:externalNotice=true()))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('CNWDI')) and not ($elem/@ism:externalNotice=true()))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
        [ISM-ID-00244][Error] If ISM_USGOV_RESOURCE and:
        1. Any element meeting ISM_CONTRIBUTES in the document has the attribute @ism:atomicEnergyMarkings containing [RD-CNWDI]
        AND
        2. No element meeting ISM_CONTRIBUTES in the document has @ism:noticeType containing [CNWDI].
        that does not have attribute @ism:externalNotice with a value of [true].
        
        Human Readable: USA documents containing CNWDI data must also have an CNWDI notice.
    </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M383"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M383"/>
   <xsl:template match="@*|node()" priority="-2" mode="M383">
      <xsl:apply-templates select="*" mode="M383"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00245-->


	<!--RULE ISM-ID-00245-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and (util:containsAnyOfTheTokens(@ism:noticeType, ('CNWDI'))) and not (@ism:externalNotice=true())]"
                 priority="1000"
                 mode="M384">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and (util:containsAnyOfTheTokens(@ism:noticeType, ('CNWDI'))) and not (@ism:externalNotice=true())]"
                       id="ISM-ID-00245-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of($partAtomicEnergyMarkings_tok, 'RD-CNWDI')&gt;0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of($partAtomicEnergyMarkings_tok, 'RD-CNWDI')&gt;0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00245][Error] If ISM_USGOV_RESOURCE and:
            1. No element without @ism:excludeFromRollup=true() in the document has the attribute @ism:atomicEnergyMarkings containing [RD-CNWDI]
            AND
            2. Any element without @ism:excludeFromRollup=true() in the document has the attribute @ism:noticeType containing [CNWDI]
            and not the attribute @ism:externalNotice with a value of [true].
            
            Human Readable: USA documents containing an CNWDI notice must also have RD-CNWDI data.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M384"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M384"/>
   <xsl:template match="@*|node()" priority="-2" mode="M384">
      <xsl:apply-templates select="*" mode="M384"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00250-->


	<!--RULE ISM-ID-00250-R1-->
<xsl:template match="ism:Notice[$ISM_USGOV_RESOURCE]" priority="1000" mode="M386">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ism:Notice[$ISM_USGOV_RESOURCE]"
                       id="ISM-ID-00250-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:noticeType or @ism:unregisteredNoticeType"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="@ism:noticeType or @ism:unregisteredNoticeType">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00250][Error] If ISM_USGOV_RESOURCE, element ism:Notice must specify 
		    	attribute @ism:noticeType or @ism:unregisteredNoticeType.
		    	
		    	Human Readable: Notices must specify their type.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M386"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M386"/>
   <xsl:template match="@*|node()" priority="-2" mode="M386">
      <xsl:apply-templates select="*" mode="M386"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00253-->


	<!--RULE ValidateTokenValuesExistenceInList-R1-->
<xsl:template match="*[@ism:atomicEnergyMarkings]" priority="1000" mode="M388">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:atomicEnergyMarkings]"
                       id="ValidateTokenValuesExistenceInList-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $searchTerm in tokenize(normalize-space(string(@ism:atomicEnergyMarkings)), ' ') satisfies                    $searchTerm = $atomicEnergyMarkingsList or (some $Term in $atomicEnergyMarkingsList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $searchTerm in tokenize(normalize-space(string(@ism:atomicEnergyMarkings)), ' ') satisfies $searchTerm = $atomicEnergyMarkingsList or (some $Term in $atomicEnergyMarkingsList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00253][Error] All @ism:atomicEnergyMarkings values must   be defined in CVEnumISMAtomicEnergyMarkings.xml.'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M388"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M388"/>
   <xsl:template match="@*|node()" priority="-2" mode="M388">
      <xsl:apply-templates select="*" mode="M388"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00254-->


	<!--RULE ValidateTokenValuesExistenceInList-R1-->
<xsl:template match="*[@ism:classification]" priority="1000" mode="M389">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:classification]"
                       id="ValidateTokenValuesExistenceInList-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $searchTerm in tokenize(normalize-space(string(@ism:classification)), ' ') satisfies                    $searchTerm = $classificationAllList or (some $Term in $classificationAllList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $searchTerm in tokenize(normalize-space(string(@ism:classification)), ' ') satisfies $searchTerm = $classificationAllList or (some $Term in $classificationAllList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'   [ISM-ID-00254][Error] All @ism:classification values must   be a defined in CVEnumISMClassificationAll.xml.   '"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M389"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M389"/>
   <xsl:template match="@*|node()" priority="-2" mode="M389">
      <xsl:apply-templates select="*" mode="M389"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00255-->


	<!--RULE ValidateTokenValuesExistenceInList-R1-->
<xsl:template match="*[@ism:exemptFrom]" priority="1000" mode="M390">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:exemptFrom]"
                       id="ValidateTokenValuesExistenceInList-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $searchTerm in tokenize(normalize-space(string(@ism:exemptFrom)), ' ') satisfies                    $searchTerm = $exemptFromList or (some $Term in $exemptFromList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $searchTerm in tokenize(normalize-space(string(@ism:exemptFrom)), ' ') satisfies $searchTerm = $exemptFromList or (some $Term in $exemptFromList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00255][Error] All @ism:exemptFrom values must be defined in CVEnumISMExemptFrom.xml.'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M390"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M390"/>
   <xsl:template match="@*|node()" priority="-2" mode="M390">
      <xsl:apply-templates select="*" mode="M390"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00256-->


	<!--RULE ValidateTokenValuesExistenceInList-R1-->
<xsl:template match="*[@ism:declassException]" priority="1000" mode="M391">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:declassException]"
                       id="ValidateTokenValuesExistenceInList-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $searchTerm in tokenize(normalize-space(string(@ism:declassException)), ' ') satisfies                    $searchTerm = $declassExceptionList or (some $Term in $declassExceptionList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $searchTerm in tokenize(normalize-space(string(@ism:declassException)), ' ') satisfies $searchTerm = $declassExceptionList or (some $Term in $declassExceptionList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'   [ISM-ID-00256][Error] All @ism:declassException values must   be defined in CVEnumISM25X.xml.   '"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M391"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M391"/>
   <xsl:template match="@*|node()" priority="-2" mode="M391">
      <xsl:apply-templates select="*" mode="M391"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00257-->


	<!--RULE ValidateTokenValuesExistenceInList-R1-->
<xsl:template match="*[@ism:displayOnlyTo]" priority="1000" mode="M392">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:displayOnlyTo]"
                       id="ValidateTokenValuesExistenceInList-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $searchTerm in tokenize(normalize-space(string(@ism:displayOnlyTo)), ' ') satisfies                    $searchTerm = $displayOnlyToList or (some $Term in $displayOnlyToList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $searchTerm in tokenize(normalize-space(string(@ism:displayOnlyTo)), ' ') satisfies $searchTerm = $displayOnlyToList or (some $Term in $displayOnlyToList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'   [ISM-ID-00257][Error] All @ism:displayOnlyTo values must   be defined in CVEnumISMCATRelTo.xml.   '"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M392"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M392"/>
   <xsl:template match="@*|node()" priority="-2" mode="M392">
      <xsl:apply-templates select="*" mode="M392"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00258-->


	<!--RULE ValidateTokenValuesExistenceInList-R1-->
<xsl:template match="*[@ism:disseminationControls]" priority="1000" mode="M393">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:disseminationControls]"
                       id="ValidateTokenValuesExistenceInList-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $searchTerm in tokenize(normalize-space(string(@ism:disseminationControls)), ' ') satisfies                    $searchTerm = $disseminationControlsList or (some $Term in $disseminationControlsList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $searchTerm in tokenize(normalize-space(string(@ism:disseminationControls)), ' ') satisfies $searchTerm = $disseminationControlsList or (some $Term in $disseminationControlsList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'   [ISM-ID-00258][Error] All @ism:disseminationControls values must   be a defined in CVEnumISMDissem.xml.   '"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M393"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M393"/>
   <xsl:template match="@*|node()" priority="-2" mode="M393">
      <xsl:apply-templates select="*" mode="M393"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00259-->


	<!--RULE ValidateTokenValuesExistenceInList-R1-->
<xsl:template match="*[@ism:FGIsourceOpen]" priority="1000" mode="M394">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:FGIsourceOpen]"
                       id="ValidateTokenValuesExistenceInList-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $searchTerm in tokenize(normalize-space(string(@ism:FGIsourceOpen)), ' ') satisfies                    $searchTerm = $FGIsourceOpenList or (some $Term in $FGIsourceOpenList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $searchTerm in tokenize(normalize-space(string(@ism:FGIsourceOpen)), ' ') satisfies $searchTerm = $FGIsourceOpenList or (some $Term in $FGIsourceOpenList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'   [ISM-ID-00259][Error] All @ism:FGIsourceOpen values must   be defined in CVEnumISMCATFGIOpen.xml.   '"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M394"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M394"/>
   <xsl:template match="@*|node()" priority="-2" mode="M394">
      <xsl:apply-templates select="*" mode="M394"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00260-->


	<!--RULE ValidateTokenValuesExistenceInList-R1-->
<xsl:template match="*[@ism:FGIsourceProtected]" priority="1000" mode="M395">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:FGIsourceProtected]"
                       id="ValidateTokenValuesExistenceInList-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $searchTerm in tokenize(normalize-space(string(@ism:FGIsourceProtected)), ' ') satisfies                    $searchTerm = $FGIsourceProtectedList or (some $Term in $FGIsourceProtectedList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $searchTerm in tokenize(normalize-space(string(@ism:FGIsourceProtected)), ' ') satisfies $searchTerm = $FGIsourceProtectedList or (some $Term in $FGIsourceProtectedList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'   [ISM-ID-00260][Error] All @ism:FGIsourceProtected values must   be defined in CVEnumISMCATFGIProtected.xml.   '"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M395"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M395"/>
   <xsl:template match="@*|node()" priority="-2" mode="M395">
      <xsl:apply-templates select="*" mode="M395"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00261-->


	<!--RULE ValidateTokenValuesExistenceInListWhenContributesToRollupACCM-R1-->
<xsl:template match="*[@ism:nonICmarkings]" priority="1000" mode="M396">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:nonICmarkings]"
                       id="ValidateTokenValuesExistenceInListWhenContributesToRollupACCM-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="if (util:contributesToRollup(.)) then every $searchTerm in tokenize(normalize-space(string(@ism:nonICmarkings)), ' ') satisfies             $searchTerm = $nonICmarkingsList or (some $Term in $nonICmarkingsList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$')))) else true()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="if (util:contributesToRollup(.)) then every $searchTerm in tokenize(normalize-space(string(@ism:nonICmarkings)), ' ') satisfies $searchTerm = $nonICmarkingsList or (some $Term in $nonICmarkingsList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$')))) else true()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00261][Error] All @ism:nonICmarkings values that contribute to rollup must be defined in CVEnumISMNonIC.xml.'"/>
                  <xsl:text/>
            The value(s) [<xsl:text/>
                  <xsl:value-of select="string-join(for $searchTerm in tokenize(normalize-space(string(@ism:nonICmarkings)), ' ')                  return if($searchTerm = $nonICmarkingsList) then null else $searchTerm,' ')"/>
                  <xsl:text/>] that contribute to rollup could not be found.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="if (not(util:contributesToRollup(.))) then every $searchTerm in tokenize(normalize-space(string(util:getStringFromSequenceWithoutRegexValues(tokenize(normalize-space(string(@ism:nonICmarkings)), ' '), $ACCMRegex))), ' ') satisfies             $searchTerm = tokenize(normalize-space(string(util:getStringFromSequenceWithoutRegexValues($nonICmarkingsList, $ACCMRegex))), ' ') or (some $Term in tokenize(normalize-space(string(util:getStringFromSequenceWithoutRegexValues($nonICmarkingsList, $ACCMRegex))), ' ') satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$')))) else true()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="if (not(util:contributesToRollup(.))) then every $searchTerm in tokenize(normalize-space(string(util:getStringFromSequenceWithoutRegexValues(tokenize(normalize-space(string(@ism:nonICmarkings)), ' '), $ACCMRegex))), ' ') satisfies $searchTerm = tokenize(normalize-space(string(util:getStringFromSequenceWithoutRegexValues($nonICmarkingsList, $ACCMRegex))), ' ') or (some $Term in tokenize(normalize-space(string(util:getStringFromSequenceWithoutRegexValues($nonICmarkingsList, $ACCMRegex))), ' ') satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$')))) else true()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00261][Error] All non-ACCM @ism:nonICmarkings values that do not contribute to rollup must be defined in CVEnumISMNonIC.xml.'"/>
                  <xsl:text/>
            The value(s) [<xsl:text/>
                  <xsl:value-of select="string-join(for $searchTerm in tokenize(normalize-space(string(util:getStringFromSequenceWithoutRegexValues(tokenize(normalize-space(string(@ism:nonICmarkings)), ' '), $ACCMRegex))), ' ')                  return if($searchTerm = tokenize(normalize-space(string(util:getStringFromSequenceWithoutRegexValues($nonICmarkingsList, $ACCMRegex))), ' ')) then null else $searchTerm,' ')"/>
                  <xsl:text/>] that contribute to rollup could not be found.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M396"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M396"/>
   <xsl:template match="@*|node()" priority="-2" mode="M396">
      <xsl:apply-templates select="*" mode="M396"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00262-->


	<!--RULE ValidateTokenValuesExistenceInList-R1-->
<xsl:template match="*[@ism:nonUSControls]" priority="1000" mode="M397">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:nonUSControls]"
                       id="ValidateTokenValuesExistenceInList-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $searchTerm in tokenize(normalize-space(string(@ism:nonUSControls)), ' ') satisfies                    $searchTerm = $nonUSControlsList or (some $Term in $nonUSControlsList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $searchTerm in tokenize(normalize-space(string(@ism:nonUSControls)), ' ') satisfies $searchTerm = $nonUSControlsList or (some $Term in $nonUSControlsList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'   [ISM-ID-00262][Error] Any @ism:nonUSControls values must   be defined in CVEnumISMNonUSControls.xml.   '"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M397"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M397"/>
   <xsl:template match="@*|node()" priority="-2" mode="M397">
      <xsl:apply-templates select="*" mode="M397"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00263-->


	<!--RULE ValidateTokenValuesExistenceInList-R1-->
<xsl:template match="*[@ism:ownerProducer]" priority="1000" mode="M398">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:ownerProducer]"
                       id="ValidateTokenValuesExistenceInList-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $searchTerm in tokenize(normalize-space(string(@ism:ownerProducer)), ' ') satisfies                    $searchTerm = $ownerProducerList or (some $Term in $ownerProducerList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $searchTerm in tokenize(normalize-space(string(@ism:ownerProducer)), ' ') satisfies $searchTerm = $ownerProducerList or (some $Term in $ownerProducerList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'   [ISM-ID-00263][Error] Any @ism:ownerProducer values must   be defined in CVEnumISMCATOwnerProducer.xml.   '"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M398"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M398"/>
   <xsl:template match="@*|node()" priority="-2" mode="M398">
      <xsl:apply-templates select="*" mode="M398"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00264-->


	<!--RULE ValidateTokenValuesExistenceInList-R1-->
<xsl:template match="*[@ism:pocType]" priority="1000" mode="M399">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:pocType]"
                       id="ValidateTokenValuesExistenceInList-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $searchTerm in tokenize(normalize-space(string(@ism:pocType)), ' ') satisfies                    $searchTerm = $pocTypeList or (some $Term in $pocTypeList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $searchTerm in tokenize(normalize-space(string(@ism:pocType)), ' ') satisfies $searchTerm = $pocTypeList or (some $Term in $pocTypeList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'   [ISM-ID-00264][Error] Any @ism:pocType values must   be defined in CVEnumISMPocType.xml.   '"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M399"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M399"/>
   <xsl:template match="@*|node()" priority="-2" mode="M399">
      <xsl:apply-templates select="*" mode="M399"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00265-->


	<!--RULE ValidateTokenValuesExistenceInList-R1-->
<xsl:template match="*[@ism:releasableTo]" priority="1000" mode="M400">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:releasableTo]"
                       id="ValidateTokenValuesExistenceInList-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $searchTerm in tokenize(normalize-space(string(@ism:releasableTo)), ' ') satisfies                    $searchTerm = $releasableToList or (some $Term in $releasableToList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $searchTerm in tokenize(normalize-space(string(@ism:releasableTo)), ' ') satisfies $searchTerm = $releasableToList or (some $Term in $releasableToList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'   [ISM-ID-00265][Error] Any @ism:releasableTo must   be a value in CVEnumISMCATRelTo.xml.   '"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M400"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M400"/>
   <xsl:template match="@*|node()" priority="-2" mode="M400">
      <xsl:apply-templates select="*" mode="M400"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00266-->


	<!--RULE ValidateTokenValuesExistenceInListWhenContributesToRollup-R1-->
<xsl:template match="*[@ism:SARIdentifier]" priority="1000" mode="M401">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:SARIdentifier]"
                       id="ValidateTokenValuesExistenceInListWhenContributesToRollup-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="if (util:contributesToRollup(.)) then every $searchTerm in tokenize(normalize-space(string(@ism:SARIdentifier)), ' ') satisfies             $searchTerm = $SARIdentifierList or (some $Term in $SARIdentifierList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$')))) else true()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="if (util:contributesToRollup(.)) then every $searchTerm in tokenize(normalize-space(string(@ism:SARIdentifier)), ' ') satisfies $searchTerm = $SARIdentifierList or (some $Term in $SARIdentifierList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$')))) else true()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00266][Error] All @ism:SARIdentifier values must be defined in CVEnumISMSAR.xml.'"/>
                  <xsl:text/>
            The value(s) [<xsl:text/>
                  <xsl:value-of select="string-join(for $searchTerm in tokenize(normalize-space(string(@ism:SARIdentifier)), ' ')                  return if($searchTerm = $SARIdentifierList) then null else $searchTerm,' ')"/>
                  <xsl:text/>] that contribute to rollup could not be found.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M401"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M401"/>
   <xsl:template match="@*|node()" priority="-2" mode="M401">
      <xsl:apply-templates select="*" mode="M401"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00267-->


	<!--RULE ValidateTokenValuesExistenceInListWhenContributesToRollup-R1-->
<xsl:template match="*[@ism:SCIcontrols]" priority="1000" mode="M402">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:SCIcontrols]"
                       id="ValidateTokenValuesExistenceInListWhenContributesToRollup-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="if (util:contributesToRollup(.)) then every $searchTerm in tokenize(normalize-space(string(@ism:SCIcontrols)), ' ') satisfies             $searchTerm = $SCIcontrolsList or (some $Term in $SCIcontrolsList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$')))) else true()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="if (util:contributesToRollup(.)) then every $searchTerm in tokenize(normalize-space(string(@ism:SCIcontrols)), ' ') satisfies $searchTerm = $SCIcontrolsList or (some $Term in $SCIcontrolsList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$')))) else true()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00267][Error] All @ism:SCIcontrols values must be defined in CVEnumISMSCIControls.xml.'"/>
                  <xsl:text/>
            The value(s) [<xsl:text/>
                  <xsl:value-of select="string-join(for $searchTerm in tokenize(normalize-space(string(@ism:SCIcontrols)), ' ')                  return if($searchTerm = $SCIcontrolsList) then null else $searchTerm,' ')"/>
                  <xsl:text/>] that contribute to rollup could not be found.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M402"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M402"/>
   <xsl:template match="@*|node()" priority="-2" mode="M402">
      <xsl:apply-templates select="*" mode="M402"/>
   </xsl:template>

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

   <!--PATTERN ISM-ID-00299-->


	<!--RULE ISM-ID-00299-R1-->
<xsl:template match="*[util:containsAnyTokenMatching(@ism:declassException, ('AEA'))]"
                 priority="1000"
                 mode="M434">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[util:containsAnyTokenMatching(@ism:declassException, ('AEA'))]"
                       id="ISM-ID-00299-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:atomicEnergyMarkings"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="@ism:atomicEnergyMarkings">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00299][Error] If an element contains the attribute @ism:declassException with a value of [AEA], 
		    	it must also contain the attribute @ism:atomicEnergyMarkings.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M434"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M434"/>
   <xsl:template match="@*|node()" priority="-2" mode="M434">
      <xsl:apply-templates select="*" mode="M434"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00302-->


	<!--RULE ISM-ID-00302-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC-USGOV'))]"
                 priority="1000"
                 mode="M435">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC-USGOV'))]"
                       id="ISM-ID-00302-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00302][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls contains the name token [OC-USGOV], then 
            name token [OC] must be specified.
            
            Human Readable: A USA document with OC-USGOV dissemination must 
            also contain an OC dissemination.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M435"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M435"/>
   <xsl:template match="@*|node()" priority="-2" mode="M435">
      <xsl:apply-templates select="*" mode="M435"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00313-->


	<!--RULE ISM-ID-00313-R1-->
<xsl:template match="*[util:containsAnyOfTheTokens(@ism:nonICmarkings, ('ND'))]"
                 priority="1000"
                 mode="M437">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[util:containsAnyOfTheTokens(@ism:nonICmarkings, ('ND'))]"
                       id="ISM-ID-00313-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00313][Error] If @ism:nonICmarkings contains the token [ND] then the 
            attribute @ism:disseminationControls must contain [NF].
            
            Human Readable: NODIS data must be marked NOFORN.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M437"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M437"/>
   <xsl:template match="@*|node()" priority="-2" mode="M437">
      <xsl:apply-templates select="*" mode="M437"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00314-->


	<!--RULE ISM-ID-00314-R1-->
<xsl:template match="*[util:containsAnyOfTheTokens(@ism:nonICmarkings, ('XD'))]"
                 priority="1000"
                 mode="M438">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[util:containsAnyOfTheTokens(@ism:nonICmarkings, ('XD'))]"
                       id="ISM-ID-00314-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00314][Error] If @ism:nonICmarkings contains the token [XD] then the 
            attribute @ism:disseminationControls must contain [NF].
            
            Human Readable: EXDIS data must be marked NOFORN.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M438"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M438"/>
   <xsl:template match="@*|node()" priority="-2" mode="M438">
      <xsl:apply-templates select="*" mode="M438"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00319-->


	<!--RULE ISM-ID-00319-R1-->
<xsl:template match="*[util:containsAnyTokenMatching(@ism:ownerProducer, 'USA') and @ism:releasableTo and $ISM_USGOV_RESOURCE]"
                 priority="1000"
                 mode="M443">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[util:containsAnyTokenMatching(@ism:ownerProducer, 'USA') and @ism:releasableTo and $ISM_USGOV_RESOURCE]"
                       id="ISM-ID-00319-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count(tokenize(normalize-space(string(@ism:releasableTo)), ' ')) &gt; 1"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count(tokenize(normalize-space(string(@ism:releasableTo)), ' ')) &gt; 1">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00319][Error] If ISM_USGOV_RESOURCE and @ism:ownerProducer contains 'USA' and attribute
            @ism:releasableTo is specified, then @ism:releasableTo must contain more than a single token.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M443"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M443"/>
   <xsl:template match="@*|node()" priority="-2" mode="M443">
      <xsl:apply-templates select="*" mode="M443"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00321-->


	<!--RULE MutuallyExclusiveAttributeValues-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD', 'FRD', 'TFNI'))]"
                 priority="1000"
                 mode="M445">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD', 'FRD', 'TFNI'))]"
                       id="MutuallyExclusiveAttributeValues-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( for $token in tokenize(normalize-space(string(@ism:atomicEnergyMarkings)),' ') return  if($token = ('RD', 'FRD', 'TFNI')) then 1 else null ) = 1"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( for $token in tokenize(normalize-space(string(@ism:atomicEnergyMarkings)),' ') return if($token = ('RD', 'FRD', 'TFNI')) then 1 else null ) = 1">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			               <xsl:text/>
                  <xsl:value-of select="'         [ISM-ID-00321][Error] If ISM_USGOV_RESOURCE, then tokens [RD],                [FRD] and [TFNI] are mutually exclusive for attribute atomicEnergyMarkings.         Human Readable: RD, FRD and TFNI are mutually exclusive and cannot be commingled         in a portion mark or in the banner line.         '"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M445"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M445"/>
   <xsl:template match="@*|node()" priority="-2" mode="M445">
      <xsl:apply-templates select="*" mode="M445"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00325-->


	<!--RULE MutuallyExclusiveAttributeValues-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC', 'RELIDO'))]"
                 priority="1000"
                 mode="M447">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC', 'RELIDO'))]"
                       id="MutuallyExclusiveAttributeValues-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( for $token in tokenize(normalize-space(string(@ism:disseminationControls)),' ') return  if($token = ('OC', 'RELIDO')) then 1 else null ) = 1"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( for $token in tokenize(normalize-space(string(@ism:disseminationControls)),' ') return if($token = ('OC', 'RELIDO')) then 1 else null ) = 1">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			               <xsl:text/>
                  <xsl:value-of select="'   [ISM-ID-00325][Error] If ISM_USGOV_RESOURCE, then tokens [OC]    and [RELIDO] are mutually exclusive for attribute disseminationControls.   '"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M447"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M447"/>
   <xsl:template match="@*|node()" priority="-2" mode="M447">
      <xsl:apply-templates select="*" mode="M447"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00327-->


	<!--RULE ISM-ID-00327-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('FOUO')) and util:containsAnyOfTheTokens(@ism:classification, ('U'))]"
                 priority="1000"
                 mode="M449">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('FOUO')) and util:containsAnyOfTheTokens(@ism:classification, ('U'))]"
                       id="ISM-ID-00327-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsOnlyTheTokens(@ism:disseminationControls, ('REL', 'RELIDO', 'NF', 'EYES', 'DISPLAYONLY', 'FOUO'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsOnlyTheTokens(@ism:disseminationControls, ('REL', 'RELIDO', 'NF', 'EYES', 'DISPLAYONLY', 'FOUO'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00327][Error]  If ISM_USGOV_RESOURCE and: 
            1. Any element in the document that has the attribute @ism:disseminationControls containing [FOUO]
            AND
            2. Has the attribute @ism:classification [U]
            Then the element can only have the @ism:disseminationControls containing [REL], [RELIDO], [NF], [DISPLAYONLY], and [EYES].
            
            Human Readable: Dissemination control markings, excluding Foreign Disclosure and Release markings 
            (REL, RELIDO, NF, DISPLAYONLY, or EYES), in elements of USA Unclassified documents supersede and take precedence 
            over FOUO.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M449"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M449"/>
   <xsl:template match="@*|node()" priority="-2" mode="M449">
      <xsl:apply-templates select="*" mode="M449"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00328-->


	<!--RULE ISM-ID-00328-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('FOUO')) and util:containsAnyOfTheTokens(@ism:classification, ('U'))]"
                 priority="1000"
                 mode="M450">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('FOUO')) and util:containsAnyOfTheTokens(@ism:classification, ('U'))]"
                       id="ISM-ID-00328-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:nonICmarkings)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="not(@ism:nonICmarkings)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00328][Error] If ISM_USGOV_RESOURCE and: 
            1. Any element in the document that has the attribute @ism:disseminationControls containing [FOUO]
            AND
            2. Has the attribute @ism:classification [U]
            
            Then the element can't have any @ism:nonICMarkings.
            
            Human Readable: Non-IC dissemination control markings in elements of USA Unclassified documents 
            supersede and take precedence over FOUO.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M450"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M450"/>
   <xsl:template match="@*|node()" priority="-2" mode="M450">
      <xsl:apply-templates select="*" mode="M450"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00330-->


	<!--RULE ISM-ID-00330-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS-P'))]"
                 priority="1000"
                 mode="M451">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS-P'))]"
                       id="ISM-ID-00330-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00330][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [HCS-P], then attribute 
            @ism:classification must have a value of [TS], or [S].
            
            Human Readable: A USA document with HCS-PRODUCT compartment data must be classified SECRET or TOP SECRET.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M451"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M451"/>
   <xsl:template match="@*|node()" priority="-2" mode="M451">
      <xsl:apply-templates select="*" mode="M451"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00332-->


	<!--RULE ISM-ID-00332-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS-O'))]"
                 priority="1000"
                 mode="M452">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS-O'))]"
                       id="ISM-ID-00332-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00332][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [HCS-O], 
            then attribute @ism:classification must have a value of [TS] or [S].
            
            Human Readable: A USA document with HCS-OPERATIONS compartment data must be classified SECRET or TOP SECRET.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M452"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M452"/>
   <xsl:template match="@*|node()" priority="-2" mode="M452">
      <xsl:apply-templates select="*" mode="M452"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00335-->


	<!--RULE ISM-ID-00335-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE  and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS-O'))]"
                 priority="1000"
                 mode="M453">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE  and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS-O'))]"
                       id="ISM-ID-00335-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00335][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [HCS-O],
            then attribute @ism:disseminationControls must contain the name token [OC].
            
            Human Readable: A USA document with HCS-OPERATIONS compartment data must be marked for 
            ORIGINATOR CONTROLLED dissemination.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M453"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M453"/>
   <xsl:template match="@*|node()" priority="-2" mode="M453">
      <xsl:apply-templates select="*" mode="M453"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00336-->


	<!--RULE ISM-ID-00336-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^HCS-P-[A-Z0-9]{1,6}$'))]"
                 priority="1000"
                 mode="M454">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^HCS-P-[A-Z0-9]{1,6}$'))]"
                       id="ISM-ID-00336-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00336][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains a token matching [HCS-P-XXXXXX], 
            where X is represented by the regular expression character class [A-Z0-9]{1,6}, then attribute
            @ism:disseminationControls must contain the name token [OC].
            
            Human Readable: A USA document with HCS-PRODUCT subcompartment data must be marked for 
            ORIGINATOR CONTROLLED dissemination.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M454"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M454"/>
   <xsl:template match="@*|node()" priority="-2" mode="M454">
      <xsl:apply-templates select="*" mode="M454"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00341-->


	<!--RULE ISM-ID-00341-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and (util:containsAnyTokenMatching(@ism:SCIcontrols, ('^SI-G-[A-Z]{4}$'))) or util:containsAnyOfTheTokens(@ism:SCIcontrols, ('SI-G'))]"
                 priority="1000"
                 mode="M455">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and (util:containsAnyTokenMatching(@ism:SCIcontrols, ('^SI-G-[A-Z]{4}$'))) or util:containsAnyOfTheTokens(@ism:SCIcontrols, ('SI-G'))]"
                       id="ISM-ID-00341-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC-USGOV')))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC-USGOV')))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00341][Error] If ISM_USGOV_RESOURCE and @ism:SCIcontrols contains a token matching [SI-G]
            or [SI-G-XXXX], then @ism:disseminationControls cannot contain [OC-USGOV].
            
            Human Readable: OC-USGOV cannot be used if SI-G or an SI-G subs are present. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M455"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M455"/>
   <xsl:template match="@*|node()" priority="-2" mode="M455">
      <xsl:apply-templates select="*" mode="M455"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00345-->


	<!--RULE ISM-ID-00345-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('EYES'))]"
                 priority="1000"
                 mode="M458">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('EYES'))]"
                       id="ISM-ID-00345-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsOnlyTheTokens(@ism:releasableTo, ('USA', 'AUS','CAN','GBR', 'NZL'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsOnlyTheTokens(@ism:releasableTo, ('USA', 'AUS','CAN','GBR', 'NZL'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[ISM-ID-00345][Error] If ISM_USGOV_RESOURCE and attribute @ism:disseminationControls contains the value [EYES], 
			@ism:releasableTo must only contain the token values of [USA], [AUS], [CAN], [GBR] or [NZL]. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M458"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M458"/>
   <xsl:template match="@*|node()" priority="-2" mode="M458">
      <xsl:apply-templates select="*" mode="M458"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00346-->


	<!--RULE ISM-ID-00346-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('DS'))]"
                 priority="1000"
                 mode="M459">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('DS'))]"
                       id="ISM-ID-00346-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:classification='U'"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:classification='U'">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[ISM-ID-00346][Error] If ISM_USGOV_RESOURCE and attribute @ism:nonICmarkings contains the name token [DS], 
			then attribute @ism:classification must have a value of [U].
			
			Human Readable: The DS (LIMDIS) nonICmarkings value in a USA document
			must only be used with a classification of UNCLASSIFIED.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M459"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M459"/>
   <xsl:template match="@*|node()" priority="-2" mode="M459">
      <xsl:apply-templates select="*" mode="M459"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00352-->


	<!--RULE NtkHasCorrespondingDataTwoTokens-R1-->
<xsl:template match="ntk:Access//ntk:AccessProfile[ntk:AccessPolicy[starts-with(., 'urn:us:gov:ic:aces:ntk:propin:')] and ($ISM_USGOV_RESOURCE or $ISM_USCUIONLY_RESOURCE)]"
                 priority="1000"
                 mode="M465">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ntk:Access//ntk:AccessProfile[ntk:AccessPolicy[starts-with(., 'urn:us:gov:ic:aces:ntk:propin:')] and ($ISM_USGOV_RESOURCE or $ISM_USCUIONLY_RESOURCE)]"
                       id="NtkHasCorrespondingDataTwoTokens-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of(($partDisseminationControls_tok,$partCuiBasic_tok,$partCuiSpecified_tok), 'PR') &gt; 0 or index-of(($bannerDisseminationControls_tok,$bannerCuiBasic_tok,$bannerCuiSpecified_tok), 'PR') &gt; 0 or index-of(($partDisseminationControls_tok,$partCuiBasic_tok,$partCuiSpecified_tok), 'PROPIN') &gt; 0 or index-of(($bannerDisseminationControls_tok,$bannerCuiBasic_tok,$bannerCuiSpecified_tok), 'PROPIN') &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of(($partDisseminationControls_tok,$partCuiBasic_tok,$partCuiSpecified_tok), 'PR') &gt; 0 or index-of(($bannerDisseminationControls_tok,$bannerCuiBasic_tok,$bannerCuiSpecified_tok), 'PR') &gt; 0 or index-of(($partDisseminationControls_tok,$partCuiBasic_tok,$partCuiSpecified_tok), 'PROPIN') &gt; 0 or index-of(($bannerDisseminationControls_tok,$bannerCuiBasic_tok,$bannerCuiSpecified_tok), 'PROPIN') &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
         [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00352'"/>
                  <xsl:text/>][error] <xsl:text/>
                  <xsl:value-of select="'PROPIN'"/>
                  <xsl:text/> NTK metadata requires that <xsl:text/>
                  <xsl:value-of select="'disseminationControls or cuiBasic or cuiSpecified'"/>
                  <xsl:text/> contain
         <xsl:text/>
                  <xsl:value-of select="'PR'"/>
                  <xsl:text/> or <xsl:text/>
                  <xsl:value-of select="'PROPIN'"/>
                  <xsl:text/> in at least one of (a) a portion that contributes to
         roll-up or (b) the banner.
      </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M465"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M465"/>
   <xsl:template match="@*|node()" priority="-2" mode="M465">
      <xsl:apply-templates select="*" mode="M465"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00353-->


	<!--RULE NtkHasCorrespondingData-R1-->
<xsl:template match="ntk:Access//ntk:AccessProfile[ntk:AccessPolicy[starts-with(., 'urn:us:gov:ic:aces:ntk:oc')] and $ISM_USGOV_RESOURCE]"
                 priority="1000"
                 mode="M466">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ntk:Access//ntk:AccessProfile[ntk:AccessPolicy[starts-with(., 'urn:us:gov:ic:aces:ntk:oc')] and $ISM_USGOV_RESOURCE]"
                       id="NtkHasCorrespondingData-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of($partDisseminationControls_tok, 'OC')&gt;0 or index-of($bannerDisseminationControls_tok, 'OC')&gt;0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of($partDisseminationControls_tok, 'OC')&gt;0 or index-of($bannerDisseminationControls_tok, 'OC')&gt;0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
         [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00353'"/>
                  <xsl:text/>][error] <xsl:text/>
                  <xsl:value-of select="'ORCON'"/>
                  <xsl:text/> NTK metadata
         requires that <xsl:text/>
                  <xsl:value-of select="'disseminationControls'"/>
                  <xsl:text/> contain <xsl:text/>
                  <xsl:value-of select="'OC'"/>
                  <xsl:text/> in at least one of (a)
         a portion that contributes to roll-up or (b) the banner.</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M466"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M466"/>
   <xsl:template match="@*|node()" priority="-2" mode="M466">
      <xsl:apply-templates select="*" mode="M466"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00354-->


	<!--RULE NtkHasCorrespondingData-R1-->
<xsl:template match="ntk:Access//ntk:AccessProfile[ntk:AccessPolicy[starts-with(., 'urn:us:gov:ic:aces:ntk:xd')] and $ISM_USGOV_RESOURCE]"
                 priority="1000"
                 mode="M467">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ntk:Access//ntk:AccessProfile[ntk:AccessPolicy[starts-with(., 'urn:us:gov:ic:aces:ntk:xd')] and $ISM_USGOV_RESOURCE]"
                       id="NtkHasCorrespondingData-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of($partNonICmarkings_tok, 'XD')&gt;0 or index-of($bannerNonICmarkings_tok, 'XD')&gt;0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of($partNonICmarkings_tok, 'XD')&gt;0 or index-of($bannerNonICmarkings_tok, 'XD')&gt;0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
         [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00354'"/>
                  <xsl:text/>][error] <xsl:text/>
                  <xsl:value-of select="'EXDIS'"/>
                  <xsl:text/> NTK metadata
         requires that <xsl:text/>
                  <xsl:value-of select="'nonICmarkings'"/>
                  <xsl:text/> contain <xsl:text/>
                  <xsl:value-of select="'XD'"/>
                  <xsl:text/> in at least one of (a)
         a portion that contributes to roll-up or (b) the banner.</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M467"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M467"/>
   <xsl:template match="@*|node()" priority="-2" mode="M467">
      <xsl:apply-templates select="*" mode="M467"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00355-->


	<!--RULE NtkHasCorrespondingData-R1-->
<xsl:template match="ntk:Access//ntk:AccessProfile[ntk:AccessPolicy[starts-with(., 'urn:us:gov:ic:aces:ntk:nd')] and $ISM_USGOV_RESOURCE]"
                 priority="1000"
                 mode="M468">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ntk:Access//ntk:AccessProfile[ntk:AccessPolicy[starts-with(., 'urn:us:gov:ic:aces:ntk:nd')] and $ISM_USGOV_RESOURCE]"
                       id="NtkHasCorrespondingData-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of($partNonICmarkings_tok, 'ND')&gt;0 or index-of($bannerNonICmarkings_tok, 'ND')&gt;0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of($partNonICmarkings_tok, 'ND')&gt;0 or index-of($bannerNonICmarkings_tok, 'ND')&gt;0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
         [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00355'"/>
                  <xsl:text/>][error] <xsl:text/>
                  <xsl:value-of select="'NODIS'"/>
                  <xsl:text/> NTK metadata
         requires that <xsl:text/>
                  <xsl:value-of select="'nonICmarkings'"/>
                  <xsl:text/> contain <xsl:text/>
                  <xsl:value-of select="'ND'"/>
                  <xsl:text/> in at least one of (a)
         a portion that contributes to roll-up or (b) the banner.</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M468"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M468"/>
   <xsl:template match="@*|node()" priority="-2" mode="M468">
      <xsl:apply-templates select="*" mode="M468"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00356-->


	<!--RULE DataHasCorrespondingNotice-R1-->
<xsl:template match="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('SSI'))]"
                 priority="1000"
                 mode="M469">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('SSI'))]"
                       id="DataHasCorrespondingNotice-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('SSI')) and not ($elem/@ism:externalNotice=true()))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('SSI')) and not ($elem/@ism:externalNotice=true()))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00356'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE, any
			element meeting ISM_CONTRIBUTES in the document has the attribute <xsl:text/>
                  <xsl:value-of select="'ism:nonICmarkings'"/>
                  <xsl:text/> containing [<xsl:text/>
                  <xsl:value-of select="'SSI'"/>
                  <xsl:text/>], then some
			element meeting ISM_CONTRIBUTES in the document MUST have attribute noticeType
			containing [<xsl:text/>
                  <xsl:value-of select="'SSI'"/>
                  <xsl:text/>].</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M469"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M469"/>
   <xsl:template match="@*|node()" priority="-2" mode="M469">
      <xsl:apply-templates select="*" mode="M469"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00357-->


	<!--RULE NoticeHasCorrespondingData-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('SSI'))]"
                 priority="1000"
                 mode="M470">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('SSI'))]"
                       id="NoticeHasCorrespondingData-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of($partNonICmarkings_tok, 'SSI') &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of($partNonICmarkings_tok, 'SSI') &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
				[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00357'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE with no CUI, and any element
			meeting ISM_CONTRIBUTES in the document has the attribute noticeType containing
				[<xsl:text/>
                  <xsl:value-of select="'SSI'"/>
                  <xsl:text/>], then some element meeting ISM_CONTRIBUTES in
			the document MUST have attribute <xsl:text/>
                  <xsl:value-of select="'nonICmarkings'"/>
                  <xsl:text/> containing
				[<xsl:text/>
                  <xsl:value-of select="'SSI'"/>
                  <xsl:text/>]. Human Readable: USA documents containing an
				<xsl:text/>
                  <xsl:value-of select="'SSI'"/>
                  <xsl:text/> notice must also have <xsl:text/>
                  <xsl:value-of select="'SSI'"/>
                  <xsl:text/> data. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M470"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M470"/>
   <xsl:template match="@*|node()" priority="-2" mode="M470">
      <xsl:apply-templates select="*" mode="M470"/>
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

   <!--PATTERN ISM-ID-00362-->


	<!--RULE ISM-ID-00362-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC-USGOV')) and @ism:SCIcontrols]"
                 priority="1000"
                 mode="M472">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC-USGOV')) and @ism:SCIcontrols]"
                       id="ISM-ID-00362-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(util:getStringFromSequenceWithOnlyRegexValues(@ism:SCIcontrols, 'HCS-P-[A-Z0-9]{1,6}'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(util:getStringFromSequenceWithOnlyRegexValues(@ism:SCIcontrols, 'HCS-P-[A-Z0-9]{1,6}'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00362][Error] HCS-P-subs cannot be used with OC-USGOV.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M472"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M472"/>
   <xsl:template match="@*|node()" priority="-2" mode="M472">
      <xsl:apply-templates select="*" mode="M472"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00363-->


	<!--RULE ISM-ID-00363-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC-USGOV')) and @ism:SCIcontrols]"
                 priority="1000"
                 mode="M473">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC-USGOV')) and @ism:SCIcontrols]"
                       id="ISM-ID-00363-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS-O')))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS-O')))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00363][Error] HCS-O cannot be used with OC-USGOV.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M473"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M473"/>
   <xsl:template match="@*|node()" priority="-2" mode="M473">
      <xsl:apply-templates select="*" mode="M473"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00368-->


	<!--RULE ISM-ID-00368-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE  and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('TK-BLFH'))]"
                 priority="1000"
                 mode="M477">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE  and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('TK-BLFH'))]"
                       id="ISM-ID-00368-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:classification, ('TS'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:classification, ('TS'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00368][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
            contains the name token [TK-BLFH], then attribute @ism:classification must have
            a value of [TS].
            
            Human Readable: A USA document containing TALENT KEYHOLE (TK) -BLUEFISH compartment data must
            be classified TOP SECRET.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M477"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M477"/>
   <xsl:template match="@*|node()" priority="-2" mode="M477">
      <xsl:apply-templates select="*" mode="M477"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00369-->


	<!--RULE ISM-ID-00369-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('TK-BLFH'))]"
                 priority="1000"
                 mode="M478">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('TK-BLFH'))]"
                       id="ISM-ID-00369-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00369][Error] If ISM_USGOV_RESOURCE and @ism:attribute SCIcontrols
            contains the name token [TK-BLFH], then attribute @ism:disseminationControls
            must contain the name token [NF].
            
            Human Readable: A USA document containing TALENT KEYHOLE (TK) -BLUEFISH compartment data must also be
            marked for NO FOREIGN dissemination.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M478"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M478"/>
   <xsl:template match="@*|node()" priority="-2" mode="M478">
      <xsl:apply-templates select="*" mode="M478"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00370-->


	<!--RULE ISM-ID-00370-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('TK-IDIT'))]"
                 priority="1000"
                 mode="M479">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('TK-IDIT'))]"
                       id="ISM-ID-00370-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00370][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
            contains the name token [TK-IDIT], then attribute @ism:disseminationControls
            must contain the name token [NF].
            
            Human Readable: A USA document containing TALENT KEYHOLE (TK) -IDITAROD compartment data must also be
            marked for NO FOREIGN dissemination.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M479"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M479"/>
   <xsl:template match="@*|node()" priority="-2" mode="M479">
      <xsl:apply-templates select="*" mode="M479"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00371-->


	<!--RULE ISM-ID-00371-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE  and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('TK-KAND'))]"
                 priority="1000"
                 mode="M480">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE  and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('TK-KAND'))]"
                       id="ISM-ID-00371-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00371][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
            contains the name token [TK-KAND], then attribute @ism:disseminationControls
            must contain the name token [NF].
            
            Human Readable: A USA document containing TALENT KEYHOLE (TK) -KANDIK compartment data must also be
            marked for NO FOREIGN dissemination.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M480"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M480"/>
   <xsl:template match="@*|node()" priority="-2" mode="M480">
      <xsl:apply-templates select="*" mode="M480"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00372-->


	<!--RULE ISM-ID-00372-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('LES-NF','SBU-NF'))]"
                 priority="1000"
                 mode="M481">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('LES-NF','SBU-NF'))]"
                       id="ISM-ID-00372-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF','REL','EYES','RELIDO','DISPLAYONLY')))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF','REL','EYES','RELIDO','DISPLAYONLY')))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00372][Error] If ISM_USGOV_RESOURCE and attribute @ism:nonICmarkings
            contains the name token [LES-NF] or [SBU-NF], then attribute @ism:disseminationControls
            must not contain the name token [NF], [REL], [EYES], [RELIDO], or [DISPLAYONLY].
            
            Human Readable: LES-NF and SBU-NF are incompatible with other Foreign Disclosure 
            and Release markings.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M481"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M481"/>
   <xsl:template match="@*|node()" priority="-2" mode="M481">
      <xsl:apply-templates select="*" mode="M481"/>
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

   <!--PATTERN ISM-ID-00384-->


	<!--RULE DataHasCorrespondingNotice-R1-->
<xsl:template match="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('RS'))]"
                 priority="1000"
                 mode="M486">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('RS'))]"
                       id="DataHasCorrespondingNotice-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('RSEN', 'IMCON_RSEN')) and not ($elem/@ism:externalNotice=true()))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('RSEN', 'IMCON_RSEN')) and not ($elem/@ism:externalNotice=true()))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00384'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE, any
			element meeting ISM_CONTRIBUTES in the document has the attribute <xsl:text/>
                  <xsl:value-of select="'disseminationControls'"/>
                  <xsl:text/> containing [<xsl:text/>
                  <xsl:value-of select="'RS'"/>
                  <xsl:text/>], then some
			element meeting ISM_CONTRIBUTES in the document MUST have attribute noticeType
			containing [<xsl:text/>
                  <xsl:value-of select="'RSEN', 'IMCON_RSEN'"/>
                  <xsl:text/>].</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M486"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M486"/>
   <xsl:template match="@*|node()" priority="-2" mode="M486">
      <xsl:apply-templates select="*" mode="M486"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00385-->


	<!--RULE ISM-ID-00385-R1-->
<xsl:template match="*[@ism:declassEvent]" priority="1000" mode="M487">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:declassEvent]"
                       id="ISM-ID-00385-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test=".[@ism:declassDate]"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test=".[@ism:declassDate]">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00385][Error]Attribute @ism:declassEvent requires use of attribute @ism:declassDate. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M487"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M487"/>
   <xsl:template match="@*|node()" priority="-2" mode="M487">
      <xsl:apply-templates select="*" mode="M487"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00386-->


	<!--RULE DataHasCorrespondingNoticeWithRegex-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and util:containsAnyTokenMatching(@ism:SCIcontrols, ('TK-.*'))]"
                 priority="1000"
                 mode="M488">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and util:containsAnyTokenMatching(@ism:SCIcontrols, ('TK-.*'))]"
                       id="DataHasCorrespondingNoticeWithRegex-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('GEOCAP')) and not ($elem/@ism:externalNotice=true()))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('GEOCAP')) and not ($elem/@ism:externalNotice=true()))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00386'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE, any
			element meeting ISM_CONTRIBUTES in the document has the attribute <xsl:text/>
                  <xsl:value-of select="'SCIcontrols'"/>
                  <xsl:text/> containing [<xsl:text/>
                  <xsl:value-of select="'TK-.*'"/>
                  <xsl:text/>], then some
			element meeting ISM_CONTRIBUTES in the document MUST have attribute noticeType
			containing [<xsl:text/>
                  <xsl:value-of select="'GEOCAP'"/>
                  <xsl:text/>].</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M488"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M488"/>
   <xsl:template match="@*|node()" priority="-2" mode="M488">
      <xsl:apply-templates select="*" mode="M488"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00387-->


	<!--RULE NoticeHasCorrespondingDataWithRegex-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not (@ism:externalNotice=true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('GEOCAP'))]"
                 priority="1000"
                 mode="M489">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not (@ism:externalNotice=true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('GEOCAP'))]"
                       id="NoticeHasCorrespondingDataWithRegex-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $dataToken in $partSCIcontrols_tok satisfies matches($dataToken, 'TK-.*')"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $dataToken in $partSCIcontrols_tok satisfies matches($dataToken, 'TK-.*')">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00387'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE and any element meeting
			ISM_CONTRIBUTES in the document has the attribute noticeType containing [<xsl:text/>
                  <xsl:value-of select="'GEOCAP'"/>
                  <xsl:text/>], then some element meeting ISM_CONTRIBUTES in the document
			MUST have attribute <xsl:text/>
                  <xsl:value-of select="'SCIcontrols'"/>
                  <xsl:text/> matching token regex [<xsl:text/>
                  <xsl:value-of select="'TK-.*'"/>
                  <xsl:text/>]. Human Readable: USA documents containing an <xsl:text/>
                  <xsl:value-of select="'GEOCAP'"/>
                  <xsl:text/> notice must also have <xsl:text/>
                  <xsl:value-of select="'TK-.*'"/>
                  <xsl:text/> data.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M489"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M489"/>
   <xsl:template match="@*|node()" priority="-2" mode="M489">
      <xsl:apply-templates select="*" mode="M489"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00388-->


	<!--RULE ISM-ID-00388-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^.*-[A-Z]'))]"
                 priority="1000"
                 mode="M490">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^.*-[A-Z]'))]"
                       id="ISM-ID-00388-R1"/>
      <xsl:variable name="allSCI" select="util:tokenize(@ism:SCIcontrols)"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $token in $allSCI satisfies (not(matches($token,'^.*-[A-Z]')) or (util:containsAnyOfTheTokens(@ism:SCIcontrols, string(util:before-last-delimeter($token,'-'))) ))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $token in $allSCI satisfies (not(matches($token,'^.*-[A-Z]')) or (util:containsAnyOfTheTokens(@ism:SCIcontrols, string(util:before-last-delimeter($token,'-'))) ))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
      [ISM-ID-00388][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains a token containing a "-" then it must also contain the token before the "-". This is to ensure 
      all compartments specify the control system and all subcompartments specify the compartment. The following token(s) do not meet this criteria (
      <xsl:text/>
                  <xsl:value-of select="for $token in $allSCI return if (not(matches($token,'^.*-[A-Z]')) or (util:containsAnyOfTheTokens(@ism:SCIcontrols, string(util:before-last-delimeter($token,'-'))) ))         then null else $token"/>
                  <xsl:text/> )
    </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M490"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M490"/>
   <xsl:template match="@*|node()" priority="-2" mode="M490">
      <xsl:apply-templates select="*" mode="M490"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00391-->


	<!--RULE DataHasCorrespondingNotice-R1-->
<xsl:template match="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('RAWFISA'))]"
                 priority="1000"
                 mode="M492">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('RAWFISA'))]"
                       id="DataHasCorrespondingNotice-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('RAWFISA')) and not ($elem/@ism:externalNotice=true()))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('RAWFISA')) and not ($elem/@ism:externalNotice=true()))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00391'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE, any
			element meeting ISM_CONTRIBUTES in the document has the attribute <xsl:text/>
                  <xsl:value-of select="'disseminationControls'"/>
                  <xsl:text/> containing [<xsl:text/>
                  <xsl:value-of select="'RAWFISA'"/>
                  <xsl:text/>], then some
			element meeting ISM_CONTRIBUTES in the document MUST have attribute noticeType
			containing [<xsl:text/>
                  <xsl:value-of select="'RAWFISA'"/>
                  <xsl:text/>].</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M492"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M492"/>
   <xsl:template match="@*|node()" priority="-2" mode="M492">
      <xsl:apply-templates select="*" mode="M492"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00392-->


	<!--RULE NoticeHasCorrespondingData-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('RAWFISA'))]"
                 priority="1000"
                 mode="M493">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('RAWFISA'))]"
                       id="NoticeHasCorrespondingData-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of($partDisseminationControls_tok, 'RAWFISA') &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of($partDisseminationControls_tok, 'RAWFISA') &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
				[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00392'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE with no CUI, and any element
			meeting ISM_CONTRIBUTES in the document has the attribute noticeType containing
				[<xsl:text/>
                  <xsl:value-of select="'RAWFISA'"/>
                  <xsl:text/>], then some element meeting ISM_CONTRIBUTES in
			the document MUST have attribute <xsl:text/>
                  <xsl:value-of select="'disseminationControls'"/>
                  <xsl:text/> containing
				[<xsl:text/>
                  <xsl:value-of select="'RAWFISA'"/>
                  <xsl:text/>]. Human Readable: USA documents containing an
				<xsl:text/>
                  <xsl:value-of select="'RAWFISA'"/>
                  <xsl:text/> notice must also have <xsl:text/>
                  <xsl:value-of select="'RAWFISA'"/>
                  <xsl:text/> data. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M493"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M493"/>
   <xsl:template match="@*|node()" priority="-2" mode="M493">
      <xsl:apply-templates select="*" mode="M493"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00393-->


	<!--RULE ISM-ID-00393-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('RAWFISA'))]"
                 priority="1000"
                 mode="M494">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('RAWFISA'))]"
                       id="ISM-ID-00393-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00393][Error] If ISM_USGOV_RESOURCE and attribute @ism:disseminationControls
            contains the name token [RAWFISA], then attribute @ism:classification must have
            a value of [TS] or [S].
            
            Human Readable: A USA document containing RAWFISA data must be classified
            SECRET or TOP SECRET.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M494"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M494"/>
   <xsl:template match="@*|node()" priority="-2" mode="M494">
      <xsl:apply-templates select="*" mode="M494"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00396-->


	<!--RULE ISM-ID-00396-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('KLM'))]"
                 priority="1000"
                 mode="M496">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('KLM'))]"
                       id="ISM-ID-00396-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
              [ISM-ID-00396][Warning]  If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [KLM], 
              then [KLM] SHOULD contain [NF]; ensure you have proper release authority from the KLM program.
              
              Human Readable: A USA document containing KLM data is usually NOFORN; ensure you have proper release
              authority from the KLM program.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M496"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M496"/>
   <xsl:template match="@*|node()" priority="-2" mode="M496">
      <xsl:apply-templates select="*" mode="M496"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00397-->


	<!--RULE ISM-ID-00397-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^KLM-[A-Z0-9]*$')) and not(util:containsAnyTokenMatching(@ism:SCIcontrols, ('^KLM-R$')))]"
                 priority="1000"
                 mode="M497">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^KLM-[A-Z0-9]*$')) and not(util:containsAnyTokenMatching(@ism:SCIcontrols, ('^KLM-R$')))]"
                       id="ISM-ID-00397-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
          [ISM-ID-00397][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
          contains a name token that complies with the pattern [KLM-] followed by any alphanumeric string, then attribute
          @ism:disseminationControls must contain the name token [OC], except for the [KLM-R] compartment which does not require [OC].
          
          Human Readable: A USA document containing a KLM compartment data must be marked for ORIGINATOR CONTROLLED (ORCON)
          dissemination, except for the KLM-R compartment which does not require ORCON dissemination.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M497"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M497"/>
   <xsl:template match="@*|node()" priority="-2" mode="M497">
      <xsl:apply-templates select="*" mode="M497"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00398-->


	<!--RULE ISM-ID-00398-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^KLM-[A-Z0-9]*-[A-Z0-9]*$'))]"
                 priority="1000"
                 mode="M498">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^KLM-[A-Z0-9]*-[A-Z0-9]*$'))]"
                       id="ISM-ID-00398-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
          [ISM-ID-00398][Error] If ISM_USGOV_RESOURCE and @ism:attribute SCIcontrols
          contains a name token that complies with the pattern [KLM-X-Y], where X and Y are any alphanumeric
          strings of any length, then attribute @ism:disseminationControls must contain the name token [OC].
          
          Human Readable: A USA document with any KLM subcompartments must be marked for ORIGINATOR CONTROLLED (ORCON) dissemination.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M498"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M498"/>
   <xsl:template match="@*|node()" priority="-2" mode="M498">
      <xsl:apply-templates select="*" mode="M498"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00441-->


	<!--RULE NoticeHasCorrespondingData-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('RSEN'))]"
                 priority="1000"
                 mode="M499">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('RSEN'))]"
                       id="NoticeHasCorrespondingData-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of($partDisseminationControls_tok, 'RS') &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of($partDisseminationControls_tok, 'RS') &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
				[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00441'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE with no CUI, and any element
			meeting ISM_CONTRIBUTES in the document has the attribute noticeType containing
				[<xsl:text/>
                  <xsl:value-of select="'RS'"/>
                  <xsl:text/>], then some element meeting ISM_CONTRIBUTES in
			the document MUST have attribute <xsl:text/>
                  <xsl:value-of select="'disseminationControls'"/>
                  <xsl:text/> containing
				[<xsl:text/>
                  <xsl:value-of select="'RS'"/>
                  <xsl:text/>]. Human Readable: USA documents containing an
				<xsl:text/>
                  <xsl:value-of select="'RS'"/>
                  <xsl:text/> notice must also have <xsl:text/>
                  <xsl:value-of select="'RS'"/>
                  <xsl:text/> data. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M499"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M499"/>
   <xsl:template match="@*|node()" priority="-2" mode="M499">
      <xsl:apply-templates select="*" mode="M499"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00442-->


	<!--RULE NoticeHasCorrespondingData-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('IMCON_RSEN'))]"
                 priority="1000"
                 mode="M500">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('IMCON_RSEN'))]"
                       id="NoticeHasCorrespondingData-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of($partDisseminationControls_tok, 'RS') &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of($partDisseminationControls_tok, 'RS') &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
				[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00442'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE with no CUI, and any element
			meeting ISM_CONTRIBUTES in the document has the attribute noticeType containing
				[<xsl:text/>
                  <xsl:value-of select="'RS'"/>
                  <xsl:text/>], then some element meeting ISM_CONTRIBUTES in
			the document MUST have attribute <xsl:text/>
                  <xsl:value-of select="'disseminationControls'"/>
                  <xsl:text/> containing
				[<xsl:text/>
                  <xsl:value-of select="'RS'"/>
                  <xsl:text/>]. Human Readable: USA documents containing an
				<xsl:text/>
                  <xsl:value-of select="'RS'"/>
                  <xsl:text/> notice must also have <xsl:text/>
                  <xsl:value-of select="'RS'"/>
                  <xsl:text/> data. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M500"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M500"/>
   <xsl:template match="@*|node()" priority="-2" mode="M500">
      <xsl:apply-templates select="*" mode="M500"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00443-->


	<!--RULE NoticeHasCorrespondingData-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('IMCON_RSEN'))]"
                 priority="1000"
                 mode="M501">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('IMCON_RSEN'))]"
                       id="NoticeHasCorrespondingData-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of($partDisseminationControls_tok, 'IMC') &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of($partDisseminationControls_tok, 'IMC') &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
				[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00443'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE with no CUI, and any element
			meeting ISM_CONTRIBUTES in the document has the attribute noticeType containing
				[<xsl:text/>
                  <xsl:value-of select="'IMC'"/>
                  <xsl:text/>], then some element meeting ISM_CONTRIBUTES in
			the document MUST have attribute <xsl:text/>
                  <xsl:value-of select="'disseminationControls'"/>
                  <xsl:text/> containing
				[<xsl:text/>
                  <xsl:value-of select="'IMC'"/>
                  <xsl:text/>]. Human Readable: USA documents containing an
				<xsl:text/>
                  <xsl:value-of select="'IMC'"/>
                  <xsl:text/> notice must also have <xsl:text/>
                  <xsl:value-of select="'IMC'"/>
                  <xsl:text/> data. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M501"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M501"/>
   <xsl:template match="@*|node()" priority="-2" mode="M501">
      <xsl:apply-templates select="*" mode="M501"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00444-->


	<!--RULE NoticeHasCorrespondingData-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('IMC'))]"
                 priority="1000"
                 mode="M502">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('IMC'))]"
                       id="NoticeHasCorrespondingData-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of($partDisseminationControls_tok, 'IMC') &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of($partDisseminationControls_tok, 'IMC') &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
				[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00444'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE with no CUI, and any element
			meeting ISM_CONTRIBUTES in the document has the attribute noticeType containing
				[<xsl:text/>
                  <xsl:value-of select="'IMC'"/>
                  <xsl:text/>], then some element meeting ISM_CONTRIBUTES in
			the document MUST have attribute <xsl:text/>
                  <xsl:value-of select="'disseminationControls'"/>
                  <xsl:text/> containing
				[<xsl:text/>
                  <xsl:value-of select="'IMC'"/>
                  <xsl:text/>]. Human Readable: USA documents containing an
				<xsl:text/>
                  <xsl:value-of select="'IMC'"/>
                  <xsl:text/> notice must also have <xsl:text/>
                  <xsl:value-of select="'IMC'"/>
                  <xsl:text/> data. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M502"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M502"/>
   <xsl:template match="@*|node()" priority="-2" mode="M502">
      <xsl:apply-templates select="*" mode="M502"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00459-->


	<!--RULE ISM-ID-00459-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS-X'))]"
                 priority="1000"
                 mode="M503">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS-X'))]"
                       id="ISM-ID-00459-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00459][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [HCS-X], 
            then attribute @ism:classification must have a value of [TS] or [S].
            
            Human Readable: A USA document with HCS-EXTERNAL compartment data must be classified SECRET or TOP SECRET.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M503"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M503"/>
   <xsl:template match="@*|node()" priority="-2" mode="M503">
      <xsl:apply-templates select="*" mode="M503"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00462-->


	<!--RULE ISM-ID-00462-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and @ism:classification='U']"
                 priority="1000"
                 mode="M506">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and @ism:classification='U']"
                       id="ISM-ID-00462-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(util:containsAnyTokenMatching(@ism:nonICmarkings, ('ACCM')))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(util:containsAnyTokenMatching(@ism:nonICmarkings, ('ACCM')))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00462][Error] If ISM_USGOV_RESOURCE and attribute @ism:classification is [U], then attribute @ism:nonICmarkings
            must not contain a name token that starts with ACCM.
            
            Human Readable: A USA document containing ACCM data must be classified CONFIDENTIAL, SECRET, or TOP SECRET.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M506"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M506"/>
   <xsl:template match="@*|node()" priority="-2" mode="M506">
      <xsl:apply-templates select="*" mode="M506"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00463-->


	<!--RULE ISM-ID-00463-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('BUR'))]"
                 priority="1000"
                 mode="M507">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('BUR'))]"
                       id="ISM-ID-00463-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
              [ISM-ID-00463][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
              contains the name token [BUR], then attribute @ism:disseminationControls
              must contain the name token [NF].
              
              Human Readable: A USA document containing BUR data must be marked
              for NO FOREIGN dissemination.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M507"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M507"/>
   <xsl:template match="@*|node()" priority="-2" mode="M507">
      <xsl:apply-templates select="*" mode="M507"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00464-->


	<!--RULE ISM-ID-00464-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('RSV'))]"
                 priority="1000"
                 mode="M508">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('RSV'))]"
                       id="ISM-ID-00464-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
              [ISM-ID-00464][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
              contains the name token [RSV], then attribute @ism:disseminationControls
              must contain the name token [NF].
              
              Human Readable: A USA document containing RSV data must be marked
              for NO FOREIGN dissemination.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M508"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M508"/>
   <xsl:template match="@*|node()" priority="-2" mode="M508">
      <xsl:apply-templates select="*" mode="M508"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00465-->


	<!--RULE ISM-ID-00465-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('BUR'))]"
                 priority="1000"
                 mode="M509">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('BUR'))]"
                       id="ISM-ID-00465-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00465][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
            contains the name token [BUR], then attribute @ism:classification must have
            a value of [TS] or [S].
            
            Human Readable: A USA document containing BUR data must be classified
            SECRET or TOP SECRET.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M509"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M509"/>
   <xsl:template match="@*|node()" priority="-2" mode="M509">
      <xsl:apply-templates select="*" mode="M509"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00466-->


	<!--RULE ISM-ID-00466-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('KLM'))]"
                 priority="1000"
                 mode="M510">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('KLM'))]"
                       id="ISM-ID-00466-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00466][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
            contains the name token [KLM], then attribute @ism:classification must have
            a value of [TS] or [S].
            
            Human Readable: A USA document containing KLM data must be classified
            SECRET or TOP SECRET.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M510"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M510"/>
   <xsl:template match="@*|node()" priority="-2" mode="M510">
      <xsl:apply-templates select="*" mode="M510"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00467-->


	<!--RULE ISM-ID-00467-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD','FRD'))]"
                 priority="1000"
                 mode="M511">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD','FRD'))]"
                       id="ISM-ID-00467-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00467][Warning] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings
            contains one of the name tokens [RD] or [FRD], then [RD] and [FRD] SHOULD contain [NF].
            In order to release [RD] or [FRD] data to a foreign partner, ensure you have established a sharing
            agreement per the AEA. 
            
            Human Readable: A USA document containing RD and/or FRD data is usually NOFORN;
            ensure you have proper release authority per the AEA. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M511"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M511"/>
   <xsl:template match="@*|node()" priority="-2" mode="M511">
      <xsl:apply-templates select="*" mode="M511"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00468-->


	<!--RULE ISM-ID-00468-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^KLM-R-[A-Z]{3}$'))]"
                 priority="1000"
                 mode="M512">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^KLM-R-[A-Z]{3}$'))]"
                       id="ISM-ID-00468-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:classification, ('TS'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:classification, ('TS'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00468][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
            contains a token starting with [KLM-R], then attribute @ism:classification must have
            a value of [TS].
            
            Human Readable: A USA document containing KLM-R subcompartment data must be classified
            TOP SECRET.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M512"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M512"/>
   <xsl:template match="@*|node()" priority="-2" mode="M512">
      <xsl:apply-templates select="*" mode="M512"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00469-->


	<!--RULE ISM-ID-00469-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^KLM-R-[A-Z]{3}$'))]"
                 priority="1000"
                 mode="M513">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^KLM-R-[A-Z]{3}$'))]"
                       id="ISM-ID-00469-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00469][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
            contains a token starting with [KLM-R], then attribute @ism:disseminationControls must contain
            the name token [NF]. 
            
            Human Readable: A USA document containing KLM-R subcompartment data
            must be marked for NO FOREIGN dissemination.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M513"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M513"/>
   <xsl:template match="@*|node()" priority="-2" mode="M513">
      <xsl:apply-templates select="*" mode="M513"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00470-->


	<!--RULE ISM-ID-00470-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and (util:containsAnyTokenMatching(@ism:SCIcontrols, ('^KLM-R-[A-Z]{3}$')))]"
                 priority="1000"
                 mode="M514">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and (util:containsAnyTokenMatching(@ism:SCIcontrols, ('^KLM-R-[A-Z]{3}$')))]"
                       id="ISM-ID-00470-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC-USGOV')))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC-USGOV')))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00470][Error] If ISM_USGOV_RESOURCE and @ism:SCIcontrols contains a
            token matching [KLM-R-XXX], then @ism:disseminationControls cannot contain
            [OC-USGOV]. 
            
            Human Readable: OC-USGOV cannot be used if KLM-R subcompartments are present. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M514"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M514"/>
   <xsl:template match="@*|node()" priority="-2" mode="M514">
      <xsl:apply-templates select="*" mode="M514"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00471-->


	<!--RULE ISM-ID-00471-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^KLM-R-[A-Z]{3}$'))]"
                 priority="1000"
                 mode="M515">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^KLM-R-[A-Z]{3}$'))]"
                       id="ISM-ID-00471-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
          [ISM-ID-00471][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
          contains a name token starting with [KLM-R-], then attribute
          @ism:disseminationControls must contain the name token [OC].
          
          Human Readable: A USA document containing KLM-R subcompartment data must be marked for ORIGINATOR CONTROLLED 
          dissemination.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M515"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M515"/>
   <xsl:template match="@*|node()" priority="-2" mode="M515">
      <xsl:apply-templates select="*" mode="M515"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00472-->


	<!--RULE ISM-ID-00472-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('MVL'))]"
                 priority="1000"
                 mode="M516">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('MVL'))]"
                       id="ISM-ID-00472-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:classification, ('TS'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:classification, ('TS'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00472][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
            contains the name token [MVL], then attribute @ism:classification must have
            a value of [TS].
            
            Human Readable: A USA document containing MVL data must be classified
            TOP SECRET.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M516"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M516"/>
   <xsl:template match="@*|node()" priority="-2" mode="M516">
      <xsl:apply-templates select="*" mode="M516"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00473-->


	<!--RULE ISM-ID-00473-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and (util:containsAnyOfTheTokens(@ism:disseminationControls, ('PR')))]"
                 priority="1000"
                 mode="M517">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and (util:containsAnyOfTheTokens(@ism:disseminationControls, ('PR')))]"
                       id="ISM-ID-00473-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('REL','RELIDO','NF','DISPLAYONLY','EYES'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('REL','RELIDO','NF','DISPLAYONLY','EYES'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
         [ISM-ID-00473][Error]  If ISM_USGOV_RESOURCE, PROPIN information (i.e. @ism:disseminationControls of the resource node 
         contains [PR]) requires explicit Foreign Disclosure &amp; Release (FD&amp;R) markings ([REL], [RELIDO], [NF], [DISPLAYONLY] 
         or [EYES]).
      </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M517"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M517"/>
   <xsl:template match="@*|node()" priority="-2" mode="M517">
      <xsl:apply-templates select="*" mode="M517"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00474-->


	<!--RULE ISM-ID-00474-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and (util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS')))]"
                 priority="1000"
                 mode="M518">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and (util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS')))]"
                       id="ISM-ID-00474-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS-O','HCS-P','HCS-X'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS-O','HCS-P','HCS-X'))">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
         [ISM-ID-00474][Warning] HCS information requires one of the HCS compartments: [HCS-O], [HCS-P] or [HCS-X]. 
         There are special exemption cases outlined in the IC Markings Register and Manual. Data marked HCS without 
         a compartment and unable to be positively determined to be O, P, or X MUST NOT be shared with entities who do 
         not have all three HCS compartments. Seek your Information System Security Manager's (ISSM’s) guidance. 
      </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M518"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M518"/>
   <xsl:template match="@*|node()" priority="-2" mode="M518">
      <xsl:apply-templates select="*" mode="M518"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00476-->


	<!--RULE ISM-ID-00476-R1-->
<xsl:template match="*[$ISM_USCUIONLY_RESOURCE]" priority="1000" mode="M520">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USCUIONLY_RESOURCE]"
                       id="ISM-ID-00476-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:SARIdentifier or @ism:SCIcontrols or @ism:atomicEnergyMarkings or @ism:FGIsourceOpen or @ism:FGIsourceProtected)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(@ism:SARIdentifier or @ism:SCIcontrols or @ism:atomicEnergyMarkings or @ism:FGIsourceOpen or @ism:FGIsourceProtected)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [ISM-ID-00476][Error] If @ism:compliesWith="USA-CUI-ONLY",
            then attributes @ism:SCIcontrols, @ism:SARIdentifier, @ism:atomicEnergyMarkings,
            @ism:FGIsourceOpen and @ism:FGIsourceProtected must not be specified. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M520"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M520"/>
   <xsl:template match="@*|node()" priority="-2" mode="M520">
      <xsl:apply-templates select="*" mode="M520"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00480-->


	<!--RULE AttributeValueDeprecatedWarning-R1-->
<xsl:template match="*[@ism:cuiBasic]" priority="1000" mode="M523">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:cuiBasic]"
                       id="AttributeValueDeprecatedWarning-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:cuiBasic), document('../../CVE/ISM/CVEnumISMCUIBasic.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:cuiBasic), document('../../CVE/ISM/CVEnumISMCUIBasic.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00480'"/>
                  <xsl:text/>][Warning] For attribute <xsl:text/>
                  <xsl:value-of select="'cuiBasic'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated(string(@ism:cuiBasic), document('../../CVE/ISM/CVEnumISMCUIBasic.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE,false())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M523"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M523"/>
   <xsl:template match="@*|node()" priority="-2" mode="M523">
      <xsl:apply-templates select="*" mode="M523"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00481-->


	<!--RULE AttributeValueDeprecatedWarning-R1-->
<xsl:template match="*[@ism:cuiSpecified]" priority="1000" mode="M524">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:cuiSpecified]"
                       id="AttributeValueDeprecatedWarning-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:cuiSpecified), document('../../CVE/ISM/CVEnumISMCUISpecified.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:cuiSpecified), document('../../CVE/ISM/CVEnumISMCUISpecified.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00481'"/>
                  <xsl:text/>][Warning] For attribute <xsl:text/>
                  <xsl:value-of select="'cuiSpecified'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated(string(@ism:cuiSpecified), document('../../CVE/ISM/CVEnumISMCUISpecified.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE,false())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M524"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M524"/>
   <xsl:template match="@*|node()" priority="-2" mode="M524">
      <xsl:apply-templates select="*" mode="M524"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00482-->


	<!--RULE AttributeValueDeprecatedError-R1-->
<xsl:template match="*[@ism:cuiBasic]" priority="1000" mode="M525">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:cuiBasic]"
                       id="AttributeValueDeprecatedError-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:cuiBasic), document('../../CVE/ISM/CVEnumISMCUIBasic.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:cuiBasic), document('../../CVE/ISM/CVEnumISMCUIBasic.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00482'"/>
                  <xsl:text/>][Error] For attribute <xsl:text/>
                  <xsl:value-of select="'cuiBasic'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated( string(@ism:cuiBasic), document('../../CVE/ISM/CVEnumISMCUIBasic.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE, true())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M525"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M525"/>
   <xsl:template match="@*|node()" priority="-2" mode="M525">
      <xsl:apply-templates select="*" mode="M525"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00483-->


	<!--RULE AttributeValueDeprecatedError-R1-->
<xsl:template match="*[@ism:cuiSpecified]" priority="1000" mode="M526">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:cuiSpecified]"
                       id="AttributeValueDeprecatedError-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:cuiSpecified), document('../../CVE/ISM/CVEnumISMCUISpecified.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:cuiSpecified), document('../../CVE/ISM/CVEnumISMCUISpecified.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00483'"/>
                  <xsl:text/>][Error] For attribute <xsl:text/>
                  <xsl:value-of select="'cuiSpecified'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated( string(@ism:cuiSpecified), document('../../CVE/ISM/CVEnumISMCUISpecified.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE, true())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M526"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M526"/>
   <xsl:template match="@*|node()" priority="-2" mode="M526">
      <xsl:apply-templates select="*" mode="M526"/>
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

   <!--PATTERN ISM-ID-00487-->


	<!--RULE DataHasCorrespondingNotice-R1-->
<xsl:template match="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:cuiSpecified, ('FISA'))]"
                 priority="1000"
                 mode="M530">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:cuiSpecified, ('FISA'))]"
                       id="DataHasCorrespondingNotice-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('FISA')) and not ($elem/@ism:externalNotice=true()))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('FISA')) and not ($elem/@ism:externalNotice=true()))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00487'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE, any
			element meeting ISM_CONTRIBUTES in the document has the attribute <xsl:text/>
                  <xsl:value-of select="'cuiSpecified'"/>
                  <xsl:text/> containing [<xsl:text/>
                  <xsl:value-of select="'FISA'"/>
                  <xsl:text/>], then some
			element meeting ISM_CONTRIBUTES in the document MUST have attribute noticeType
			containing [<xsl:text/>
                  <xsl:value-of select="'FISA'"/>
                  <xsl:text/>].</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M530"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M530"/>
   <xsl:template match="@*|node()" priority="-2" mode="M530">
      <xsl:apply-templates select="*" mode="M530"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00488-->


	<!--RULE NoticeHasCorrespondingCUIData-R1-->
<xsl:template match="*[$ISM_USCUIONLY_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('FISA'))]"
                 priority="1000"
                 mode="M531">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USCUIONLY_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('FISA'))]"
                       id="NoticeHasCorrespondingCUIData-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of($partCuiSpecified_tok, 'FISA') &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of($partCuiSpecified_tok, 'FISA') &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
				[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00488'"/>
                  <xsl:text/>][Error] If ISM_USCUIONLY_RESOURCE and any element
			meeting ISM_CONTRIBUTES in the document has the attribute noticeType containing
				[<xsl:text/>
                  <xsl:value-of select="'FISA'"/>
                  <xsl:text/>], then some element meeting ISM_CONTRIBUTES in
			the document MUST have attribute <xsl:text/>
                  <xsl:value-of select="'cuiSpecified'"/>
                  <xsl:text/> containing
				[<xsl:text/>
                  <xsl:value-of select="'FISA'"/>
                  <xsl:text/>].
			
			Human Readable: USA documents containing an <xsl:text/>
                  <xsl:value-of select="'FISA'"/>
                  <xsl:text/> notice must also 
			have <xsl:text/>
                  <xsl:value-of select="'FISA'"/>
                  <xsl:text/> data. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M531"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M531"/>
   <xsl:template match="@*|node()" priority="-2" mode="M531">
      <xsl:apply-templates select="*" mode="M531"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00491-->


	<!--RULE DataHasCorrespondingNotice-R1-->
<xsl:template match="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:cuiSpecified, ('SSI'))]"
                 priority="1000"
                 mode="M532">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)   and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:cuiSpecified, ('SSI'))]"
                       id="DataHasCorrespondingNotice-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('SSI')) and not ($elem/@ism:externalNotice=true()))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('SSI')) and not ($elem/@ism:externalNotice=true()))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00491'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE, any
			element meeting ISM_CONTRIBUTES in the document has the attribute <xsl:text/>
                  <xsl:value-of select="'cuiSpecified'"/>
                  <xsl:text/> containing [<xsl:text/>
                  <xsl:value-of select="'SSI'"/>
                  <xsl:text/>], then some
			element meeting ISM_CONTRIBUTES in the document MUST have attribute noticeType
			containing [<xsl:text/>
                  <xsl:value-of select="'SSI'"/>
                  <xsl:text/>].</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M532"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M532"/>
   <xsl:template match="@*|node()" priority="-2" mode="M532">
      <xsl:apply-templates select="*" mode="M532"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00492-->


	<!--RULE NoticeHasCorrespondingCUIData-R1-->
<xsl:template match="*[$ISM_USCUIONLY_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('SSI'))]"
                 priority="1000"
                 mode="M533">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USCUIONLY_RESOURCE and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ('SSI'))]"
                       id="NoticeHasCorrespondingCUIData-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of($partCuiSpecified_tok, 'SSI') &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of($partCuiSpecified_tok, 'SSI') &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
				[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00492'"/>
                  <xsl:text/>][Error] If ISM_USCUIONLY_RESOURCE and any element
			meeting ISM_CONTRIBUTES in the document has the attribute noticeType containing
				[<xsl:text/>
                  <xsl:value-of select="'SSI'"/>
                  <xsl:text/>], then some element meeting ISM_CONTRIBUTES in
			the document MUST have attribute <xsl:text/>
                  <xsl:value-of select="'cuiSpecified'"/>
                  <xsl:text/> containing
				[<xsl:text/>
                  <xsl:value-of select="'SSI'"/>
                  <xsl:text/>].
			
			Human Readable: USA documents containing an <xsl:text/>
                  <xsl:value-of select="'SSI'"/>
                  <xsl:text/> notice must also 
			have <xsl:text/>
                  <xsl:value-of select="'SSI'"/>
                  <xsl:text/> data. 
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M533"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M533"/>
   <xsl:template match="@*|node()" priority="-2" mode="M533">
      <xsl:apply-templates select="*" mode="M533"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00495-->


	<!--RULE ISM-ID-00495-R1-->
<xsl:template match="*[@ism:* and $ISM_USCUIONLY_RESOURCE]"
                 priority="1000"
                 mode="M535">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:* and $ISM_USCUIONLY_RESOURCE]"
                       id="ISM-ID-00495-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:classification or @ism:ownerProducer)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(@ism:classification or @ism:ownerProducer)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00495][Error] If @ism:compliesWith="USA-CUI-ONLY" then attributes
            @ism:classification and @ism:ownerProducer must not be specified. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M535"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M535"/>
   <xsl:template match="@*|node()" priority="-2" mode="M535">
      <xsl:apply-templates select="*" mode="M535"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00505-->


	<!--RULE ValidateTokenValuesExistenceInList-R1-->
<xsl:template match="*[@ism:cuiBasic]" priority="1000" mode="M545">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:cuiBasic]"
                       id="ValidateTokenValuesExistenceInList-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $searchTerm in tokenize(normalize-space(string(@ism:cuiBasic)), ' ') satisfies                    $searchTerm = $cuiBasicList or (some $Term in $cuiBasicList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $searchTerm in tokenize(normalize-space(string(@ism:cuiBasic)), ' ') satisfies $searchTerm = $cuiBasicList or (some $Term in $cuiBasicList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00505][Error] All @ism:cuiBasic values must   be defined in CVEnumISMCUIBasic.xml.'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M545"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M545"/>
   <xsl:template match="@*|node()" priority="-2" mode="M545">
      <xsl:apply-templates select="*" mode="M545"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00506-->


	<!--RULE ValidateTokenValuesExistenceInList-R1-->
<xsl:template match="*[@ism:cuiSpecified]" priority="1000" mode="M546">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:cuiSpecified]"
                       id="ValidateTokenValuesExistenceInList-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $searchTerm in tokenize(normalize-space(string(@ism:cuiSpecified)), ' ') satisfies                    $searchTerm = $cuiSpecifiedList or (some $Term in $cuiSpecifiedList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $searchTerm in tokenize(normalize-space(string(@ism:cuiSpecified)), ' ') satisfies $searchTerm = $cuiSpecifiedList or (some $Term in $cuiSpecifiedList satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00506][Error] All @ism:cuiSpecified values must   be defined in CVEnumISMCUISpecified.xml.'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M546"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M546"/>
   <xsl:template match="@*|node()" priority="-2" mode="M546">
      <xsl:apply-templates select="*" mode="M546"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00507-->


	<!--RULE ISM-ID-00507-R1-->
<xsl:template match="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and              util:containsAnyOfTheTokens(@ism:disseminationControls, ('AC','AWP'))]"
                 priority="1000"
                 mode="M547">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and              util:containsAnyOfTheTokens(@ism:disseminationControls, ('AC','AWP'))]"
                       id="ISM-ID-00507-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:cuiBasic, ('PRIVILEGE'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:cuiBasic, ('PRIVILEGE'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
              [ISM-ID-00507][Error] If (ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE) and attribute @ism:disseminationControls
              contains one or more of the name tokens [AC] or [AWP], then attribute @ism:cuiBasic
              must contain the name token [PRIVILEGE].
              
              Human Readable: A CUI document marked one or more of [AC] Attorney-Client and/or [AWP] Attorney Work Product
              must be marked with the CUI Basic Category Marking of PRIVILEGE.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M547"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M547"/>
   <xsl:template match="@*|node()" priority="-2" mode="M547">
      <xsl:apply-templates select="*" mode="M547"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00518-->


	<!--RULE ISM-ID-00518-R1-->
<xsl:template match="ism:Notice[$ISM_USGOV_RESOURCE and exists(@ism:noticeProseID)] | ism:NoticeExternal[$ISM_USGOV_RESOURCE and exists(@ism:noticeProseID)]"
                 priority="1000"
                 mode="M554">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ism:Notice[$ISM_USGOV_RESOURCE and exists(@ism:noticeProseID)] | ism:NoticeExternal[$ISM_USGOV_RESOURCE and exists(@ism:noticeProseID)]"
                       id="ISM-ID-00518-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(exists(.//ism:NoticeText))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(exists(.//ism:NoticeText))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00518][Error] For ism:Notice or ism:NoticeExternal, if @ism:noticeProseID is present then ism:NoticeText is prohibited.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M554"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M554"/>
   <xsl:template match="@*|node()" priority="-2" mode="M554">
      <xsl:apply-templates select="*" mode="M554"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00519-->


	<!--RULE ISM-ID-00519-R1-->
<xsl:template match="ism:Notice[$ISM_USGOV_RESOURCE and not(exists(@ism:noticeProseID))] | ism:NoticeExternal[$ISM_USGOV_RESOURCE and not(exists(@ism:noticeProseID))]"
                 priority="1000"
                 mode="M555">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ism:Notice[$ISM_USGOV_RESOURCE and not(exists(@ism:noticeProseID))] | ism:NoticeExternal[$ISM_USGOV_RESOURCE and not(exists(@ism:noticeProseID))]"
                       id="ISM-ID-00519-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="exists(.//ism:NoticeText)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="exists(.//ism:NoticeText)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00519][Error] For ism:Notice or ism:NoticeExternal, if @ism:noticeProseID is absent then ism:NoticeText is required.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M555"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M555"/>
   <xsl:template match="@*|node()" priority="-2" mode="M555">
      <xsl:apply-templates select="*" mode="M555"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00522-->


	<!--RULE ISM-ID-00522-R1-->
<xsl:template match="*[$ISM_NSI_EO_APPLIES and @ism:highWaterNATO]"
                 priority="1000"
                 mode="M557">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_NSI_EO_APPLIES and @ism:highWaterNATO]"
                       id="ISM-ID-00522-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="                 (matches(normalize-space(string(@ism:ownerProducer)), 'NATO') or                 matches(normalize-space(string(@ism:FGIsourceOpen)), 'NATO') or                 matches(normalize-space(string(@ism:FGIsourceProtected)), 'FGI'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="(matches(normalize-space(string(@ism:ownerProducer)), 'NATO') or matches(normalize-space(string(@ism:FGIsourceOpen)), 'NATO') or matches(normalize-space(string(@ism:FGIsourceProtected)), 'FGI'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [ISM-ID-00522][Error] If ISM_NSI_EO_APPLIES and the
            @ism:highWaterNATO attribute exists on a banner or portion, then @ism:FGIsourceOpen
            contains [NATO] or @ism:ownerProducer contains [NATO] or @ism:FGIsourceProtected
            contains [FGI]. Human Readable: For documents under E.O. 13526, the NATO high-water
            indicator can only exist on an element where either @ism:FGIsourceOpen contains [NATO]
            or @ism:ownerProducer contains [NATO] or @ism:FGIsourceProtected contains [FGI].
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M557"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M557"/>
   <xsl:template match="@*|node()" priority="-2" mode="M557">
      <xsl:apply-templates select="*" mode="M557"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00523-->


	<!--RULE ISM-ID-00523-R1-->
<xsl:template match="*[$ISM_NSI_EO_APPLIES and contains(@ism:FGIsourceOpen,'NATO')]"
                 priority="1000"
                 mode="M558">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_NSI_EO_APPLIES and contains(@ism:FGIsourceOpen,'NATO')]"
                       id="ISM-ID-00523-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:highWaterNATO"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:highWaterNATO">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [ISM-ID-00523][Error] If ISM_NSI_EO_APPLIES and @ism:FGIsourceOpen
                contains [NATO] on a banner or portion, then @ism:highWaterNATO must be present.
                Human Readable: For documents under E.O. 13526, the NATO high-water indicator must exist on an
                element when @ism:FGIsourceOpen contains [NATO] </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M558"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M558"/>
   <xsl:template match="@*|node()" priority="-2" mode="M558">
      <xsl:apply-templates select="*" mode="M558"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00524-->


	<!--RULE ISM-ID-00524-R1-->
<xsl:template match="*[$ISM_NSI_EO_APPLIES and @ism:highWaterNATO]"
                 priority="1000"
                 mode="M559">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_NSI_EO_APPLIES and @ism:highWaterNATO]"
                       id="ISM-ID-00524-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(./@ism:ownerProducer='NATO')"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(./@ism:ownerProducer='NATO')">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [ISM-ID-00524][Error] If ISM_NSI_EO_APPLIES and the
            @ism:highWaterNATO attribute exists on a banner or portion, then @ism:ownerProducer
            cannot be equal to 'NATO'. Human Readable: For documents under E.O. 13526, the NATO
            high-water indicator is not allowed where @ism:ownerProducer='NATO'. It is okay if
            @ism:ownerProducer contains 'NATO' and other tokens like 'USA'. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M559"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M559"/>
   <xsl:template match="@*|node()" priority="-2" mode="M559">
      <xsl:apply-templates select="*" mode="M559"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00525-->


	<!--RULE ISM-ID-00525-R1-->
<xsl:template match="*[$ISM_NSI_EO_APPLIES and @ism:highWaterNATO]"
                 priority="1000"
                 mode="M560">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_NSI_EO_APPLIES and @ism:highWaterNATO]"
                       id="ISM-ID-00525-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="if (normalize-space(string(./@ism:highWaterNATO))='NATO-TS' and normalize-space(string(./@ism:classification))='TS')                 then true()             else if (normalize-space(string(./@ism:highWaterNATO))='NATO-S' and (normalize-space(string(./@ism:classification))='S'             or normalize-space(string(./@ism:classification))='TS'))                 then true()             else if (normalize-space(string(./@ism:highWaterNATO))='NATO-C' and (normalize-space(string(./@ism:classification))='TS'              or normalize-space(string(./@ism:classification))='S' or normalize-space(string(./@ism:classification))='C'))                 then true()             else if (normalize-space(string(./@ism:highWaterNATO))='NATO-R' and not(normalize-space(string(./@ism:classification))='U'))                 then true()             else if (normalize-space(string(./@ism:highWaterNATO))='NATO-U') then true()             else false()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="if (normalize-space(string(./@ism:highWaterNATO))='NATO-TS' and normalize-space(string(./@ism:classification))='TS') then true() else if (normalize-space(string(./@ism:highWaterNATO))='NATO-S' and (normalize-space(string(./@ism:classification))='S' or normalize-space(string(./@ism:classification))='TS')) then true() else if (normalize-space(string(./@ism:highWaterNATO))='NATO-C' and (normalize-space(string(./@ism:classification))='TS' or normalize-space(string(./@ism:classification))='S' or normalize-space(string(./@ism:classification))='C')) then true() else if (normalize-space(string(./@ism:highWaterNATO))='NATO-R' and not(normalize-space(string(./@ism:classification))='U')) then true() else if (normalize-space(string(./@ism:highWaterNATO))='NATO-U') then true() else false()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [ISM-ID-00525][Error] If ISM_NSI_EO_APPLIES and the @ism:highWaterNATO
            attribute exists on a banner or portion, then @ism:highWaterNATO cannot be higher than @ism:classification.
            Human Readable: For documents under E.O. 13526, the NATO high-water indicator value cannot be
            higher than the classification value. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M560"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M560"/>
   <xsl:template match="@*|node()" priority="-2" mode="M560">
      <xsl:apply-templates select="*" mode="M560"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00526-->


	<!--RULE ISM-ID-00526-R1-->
<xsl:template match="*[$ISM_NSI_EO_APPLIES and contains(@ism:ownerProducer,'NATO') and not(@ism:ownerProducer='NATO')]"
                 priority="1000"
                 mode="M561">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_NSI_EO_APPLIES and contains(@ism:ownerProducer,'NATO') and not(@ism:ownerProducer='NATO')]"
                       id="ISM-ID-00526-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:highWaterNATO"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:highWaterNATO">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [ISM-ID-00526][Error] If ISM_NSI_EO_APPLIES and @ism:ownerProducer
            attribute contains multiple values on a banner or portion, one being NATO, then @ism:highWaterNATO must exist.
            Human Readable: For documents under E.O. 13526, the NATO high-water indicator must exist on an
            element when @ism:ownerProducer attribute contains multiple values and NATO. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M561"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M561"/>
   <xsl:template match="@*|node()" priority="-2" mode="M561">
      <xsl:apply-templates select="*" mode="M561"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00532-->


	<!--RULE ISM-ID-00532-R1-->
<xsl:template match="*[@ism:SARIdentifier]" priority="1000" mode="M566">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:SARIdentifier]"
                       id="ISM-ID-00532-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="if (contains(string(./@ism:SARIdentifier),':TS:')) then                   (if (normalize-space(string(./@ism:classification))='TS') then true() else false() )              else if (contains(string(./@ism:SARIdentifier),':S:')) then                  (if ((normalize-space(string(./@ism:classification))='S' or normalize-space(string(./@ism:classification))='TS')) then true()                     else false() )             else if (contains(string(./@ism:SARIdentifier),':C:')) then                  (if ((normalize-space(string(./@ism:classification))='TS' or normalize-space(string(./@ism:classification))='S'                      or normalize-space(string(./@ism:classification))='C')) then true() else false() )             else if (not(contains(./@ism:SARIdentifier,':TS:') or contains(string(./@ism:SARIdentifier),':S:') or                     contains(./@ism:SARIdentifier,':C:'))) then true()                             else false()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="if (contains(string(./@ism:SARIdentifier),':TS:')) then (if (normalize-space(string(./@ism:classification))='TS') then true() else false() ) else if (contains(string(./@ism:SARIdentifier),':S:')) then (if ((normalize-space(string(./@ism:classification))='S' or normalize-space(string(./@ism:classification))='TS')) then true() else false() ) else if (contains(string(./@ism:SARIdentifier),':C:')) then (if ((normalize-space(string(./@ism:classification))='TS' or normalize-space(string(./@ism:classification))='S' or normalize-space(string(./@ism:classification))='C')) then true() else false() ) else if (not(contains(./@ism:SARIdentifier,':TS:') or contains(string(./@ism:SARIdentifier),':S:') or contains(./@ism:SARIdentifier,':C:'))) then true() else false()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>[ISM-ID-00532][Error] For all elements with ism:SARIdentifier with tokens
            that include classification portion marks (e.g., DOD:TS:aaaa or DOD:C:bbbb), the value of
            the classification portion mark cannot be higher than @ism:classification on the same
            element. Human Readable: For @ism:SARIdentifier tokens that include classification portion
            marks in their values, the classification portion mark cannot be higher than the
            classification value. Note that some @ism:SARIdentifier tokens may not contain
            classification portion marks, e.g., DNI:kkkk; the rule does not apply to these tokens. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M566"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M566"/>
   <xsl:template match="@*|node()" priority="-2" mode="M566">
      <xsl:apply-templates select="*" mode="M566"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00535-->


	<!--RULE ISM-ID-00535-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('WAIVED'))]"
                 priority="1000"
                 mode="M569">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('WAIVED'))]"
                       id="ISM-ID-00535-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="$ISM_USDOD_RESOURCE"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="$ISM_USDOD_RESOURCE">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00535][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls contains the name token [WAIVED], then 
            attribute @ism:compliesWith must contain [USDOD]. 
            Human Readable: USA documents containing the WAIVED 
            dissemination control must comply with USDOD rules.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M569"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M569"/>
   <xsl:template match="@*|node()" priority="-2" mode="M569">
      <xsl:apply-templates select="*" mode="M569"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00119-->


	<!--RULE ISM-ID-00119-R1-->
<xsl:template match="*[@ism:* except (@ism:pocType | @ism:DESVersion | @ism:ISMCATCESVersion | @ism:unregisteredNoticeType)                        and $ISM_USIC_RESOURCE                        and util:contributesToRollup(.)                        and not($ISM_710_FDR_EXEMPT)                        and not(@ism:classification='U')]"
                 priority="1000"
                 mode="M570">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:* except (@ism:pocType | @ism:DESVersion | @ism:ISMCATCESVersion | @ism:unregisteredNoticeType)                        and $ISM_USIC_RESOURCE                        and util:contributesToRollup(.)                        and not($ISM_710_FDR_EXEMPT)                        and not(@ism:classification='U')]"
                       id="ISM-ID-00119-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('DISPLAYONLY', 'RELIDO','REL','EYES', 'NF'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('DISPLAYONLY', 'RELIDO','REL','EYES', 'NF'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00119][Error] If ISM_USIC_RESOURCE and 
            1. attribute @ism:classification is not [U]
            AND
            2. not ISM_710_FDR_EXEMPT
            AND
            3. attribute @ism:excludeFromRollup is not true
            AND
            4. attribute @ism:disseminationControls must contain one or more of 
            [DISPLAYONLY], [REL], [RELIDO], [EYES], or [NF].
            
            Human Readable: All classified NSI that does not claim exemption from
            ICD 710 mandatory Foreign Disclosure and Release must have an 
            appropriate foreign disclosure or release marking.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M570"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M570"/>
   <xsl:template match="@*|node()" priority="-2" mode="M570">
      <xsl:apply-templates select="*" mode="M570"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00225-->


	<!--RULE ISM-ID-00225-R1-->
<xsl:template match="*[$ISM_USIC_RESOURCE and @ism:nonICmarkings and util:contributesToRollup(.)]"
                 priority="1000"
                 mode="M571">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USIC_RESOURCE and @ism:nonICmarkings and util:contributesToRollup(.)]"
                       id="ISM-ID-00225-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(util:containsAnyTokenMatching(@ism:nonICmarkings, ('ACCM', 'NNPI')))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(util:containsAnyTokenMatching(@ism:nonICmarkings, ('ACCM', 'NNPI')))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00225][Error]  If subject to IC rules, then attribute @ism:nonICmarkings must NOT be specified 
            with a value containing any name token starting with [ACCM] or [NNPI]. 
            
            Human Readable: ACCM and NNPI tokens are not valid for documents that are subject to IC rules.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M571"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M571"/>
   <xsl:template match="@*|node()" priority="-2" mode="M571">
      <xsl:apply-templates select="*" mode="M571"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00251-->


	<!--RULE ISM-ID-00251-R1-->
<xsl:template match="*[$ISM_USIC_RESOURCE and @ism:noticeType]"
                 priority="1000"
                 mode="M572">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USIC_RESOURCE and @ism:noticeType]"
                       id="ISM-ID-00251-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(util:containsAnyTokenMatching(@ism:noticeType, 'COMSEC'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(util:containsAnyTokenMatching(@ism:noticeType, 'COMSEC'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00251][Error] If ISM_USIC_RESOURCE, then attribute @ism:noticeType must not be specified with a value of [COMSEC]. 
            
            Human Readable: COMSEC notices are not valid for US IC documents.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M572"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M572"/>
   <xsl:template match="@*|node()" priority="-2" mode="M572">
      <xsl:apply-templates select="*" mode="M572"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00002-->


	<!--RULE ISM-ID-00002-R1-->
<xsl:template match="*[@ism:*]" priority="1000" mode="M573">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:*]"
                       id="ISM-ID-00002-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $attribute in @ism:* satisfies normalize-space(string($attribute))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $attribute in @ism:* satisfies normalize-space(string($attribute))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00002][Error] For every attribute in the ISM namespace that is used in a document, a non-null value must be present.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M573"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M573"/>
   <xsl:template match="@*|node()" priority="-2" mode="M573">
      <xsl:apply-templates select="*" mode="M573"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00012-->


	<!--RULE ISM-ID-00012-R1-->
<xsl:template match="*[((@ism:* except (@ism:pocType | @ism:DESVersion | @ism:unregisteredNoticeType | @ism:ISMCATCESVersion))          or (self::arh:ExternalSecurity or self::ntk:Access or self::ntk:ExternalAccess or self::ntk:AccessProfile))         and not($ISM_USCUIONLY_RESOURCE)]"
                 priority="1000"
                 mode="M574">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[((@ism:* except (@ism:pocType | @ism:DESVersion | @ism:unregisteredNoticeType | @ism:ISMCATCESVersion))          or (self::arh:ExternalSecurity or self::ntk:Access or self::ntk:ExternalAccess or self::ntk:AccessProfile))         and not($ISM_USCUIONLY_RESOURCE)]"
                       id="ISM-ID-00012-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:ownerProducer and @ism:classification"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="@ism:ownerProducer and @ism:classification">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00012][Error] If the document does NOT have @ism:compliesWith="USA-CUI-ONLY",
            then if:
            1. any of the attributes defined in this DES other than @ism:DESVersion, @ism:ISMCATCESVersion,
            @ism:unregisteredNoticeType, or @ism:pocType are specified for an element, 
            OR 
            2. the current node is one of elements arh:Security, arh:ExternalSecurity, ntk:Access, or ntk:AccessProfile,
            then attributes @ism:classification and @ism:ownerProducer must be specified for the element.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M574"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M574"/>
   <xsl:template match="@*|node()" priority="-2" mode="M574">
      <xsl:apply-templates select="*" mode="M574"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00102-->


	<!--RULE ISM-ID-00102-R1-->
<xsl:template match="/*[descendant-or-self::*[@ism:* except (@ism:ISMCATCESVersion)]]"
                 priority="1000"
                 mode="M575">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="/*[descendant-or-self::*[@ism:* except (@ism:ISMCATCESVersion)]]"
                       id="ISM-ID-00102-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $element in descendant-or-self::node() satisfies $element/@ism:DESVersion"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $element in descendant-or-self::node() satisfies $element/@ism:DESVersion">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00102][Error] The attribute @ism:DESVersion in the namespace urn:us:gov:ic:ism must be specified.
            
            Human Readable: The data encoding specification version must be specified.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M575"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M575"/>
   <xsl:template match="@*|node()" priority="-2" mode="M575">
      <xsl:apply-templates select="*" mode="M575"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00103-->


	<!--RULE ISM-ID-00103-R1-->
<xsl:template match="/*[descendant-or-self::*[@ism:* except (@ism:ISMCATCESVersion)]]"
                 priority="1000"
                 mode="M576">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="/*[descendant-or-self::*[@ism:* except (@ism:ISMCATCESVersion)]]"
                       id="ISM-ID-00103-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $token in //*[(@ism:*)] satisfies               $token/@ism:resourceElement=true()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $token in //*[(@ism:*)] satisfies $token/@ism:resourceElement=true()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
        	[ISM-ID-00103][Error] At least one element must have attribute @ism:resourceElement specified with a value of [true].
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M576"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M576"/>
   <xsl:template match="@*|node()" priority="-2" mode="M576">
      <xsl:apply-templates select="*" mode="M576"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00163-->


	<!--RULE ISM-ID-00163-R1-->
<xsl:template match="*[@ism:nonUSControls]" priority="1000" mode="M579">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:nonUSControls]"
                       id="ISM-ID-00163-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="(matches(normalize-space(string(@ism:ownerProducer)), '^NATO:?') or matches(normalize-space(string(@ism:FGIsourceOpen)), 'NATO:?')) or @ism:FGIsourceProtected"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="(matches(normalize-space(string(@ism:ownerProducer)), '^NATO:?') or matches(normalize-space(string(@ism:FGIsourceOpen)), 'NATO:?')) or @ism:FGIsourceProtected">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00163][Error] If attribute @ism:nonUSControls exists either 
            1. the attribute @ism:ownerProducer must equal [NATO] or a [NATO:NAC] 
            OR 
            2. the attribute @ism:FGIsourceOpen must contain [NATO] or a [NATO:NAC]
            OR
            3. the attribute @ism:FGIsourceProtected is used (This should only be the case when it is a resource level or super portion marking)
            
            Human Readable: NATO and NATO/NACs are the only owner of classification markings for which nonUSControls are currently authorized.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M579"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M579"/>
   <xsl:template match="@*|node()" priority="-2" mode="M579">
      <xsl:apply-templates select="*" mode="M579"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00194-->


	<!--RULE AttributeValueDeprecatedWarning-R1-->
<xsl:template match="*[@ism:noticeType]" priority="1000" mode="M580">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:noticeType]"
                       id="AttributeValueDeprecatedWarning-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:noticeType), document('../../CVE/ISM/CVEnumISMNotice.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:noticeType), document('../../CVE/ISM/CVEnumISMNotice.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, false()))=0">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00194'"/>
                  <xsl:text/>][Warning] For attribute <xsl:text/>
                  <xsl:value-of select="'noticeType'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated(string(@ism:noticeType), document('../../CVE/ISM/CVEnumISMNotice.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE,false())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M580"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M580"/>
   <xsl:template match="@*|node()" priority="-2" mode="M580">
      <xsl:apply-templates select="*" mode="M580"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00195-->


	<!--RULE AttributeValueDeprecatedError-R1-->
<xsl:template match="*[@ism:noticeType]" priority="1000" mode="M581">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:noticeType]"
                       id="AttributeValueDeprecatedError-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count( dvf:deprecated( string(@ism:noticeType), document('../../CVE/ISM/CVEnumISMNotice.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count( dvf:deprecated( string(@ism:noticeType), document('../../CVE/ISM/CVEnumISMNotice.xml')//cve:CVE/cve:Enumeration/cve:Term[./@deprecated], $ISM_RESOURCE_CREATE_DATE, true()) )=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00195'"/>
                  <xsl:text/>][Error] For attribute <xsl:text/>
                  <xsl:value-of select="'noticeType'"/>
                  <xsl:text/>, value(s) <xsl:text/>
                  <xsl:value-of select="dvf:deprecated( string(@ism:noticeType), document('../../CVE/ISM/CVEnumISMNotice.xml')//cve:CVE/cve:Enumeration/cve:Term[@deprecated], $ISM_RESOURCE_CREATE_DATE, true())"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M581"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M581"/>
   <xsl:template match="@*|node()" priority="-2" mode="M581">
      <xsl:apply-templates select="*" mode="M581"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00376-->


	<!--RULE ISM-ID-00376-R1-->
<xsl:template match="*[@ism:ownerProducer]" priority="1000" mode="M594">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:ownerProducer]"
                       id="ISM-ID-00376-R1"/>
      <xsl:variable name="releasableToCountries"
                    select="distinct-values(for $value in tokenize(normalize-space(@ism:releasableTo),' ') return      if(index-of($catt//catt:TetraToken,$value)&gt;0)      then util:tokenize(util:getTetragraphMembership($value))      else $value)"/>
      <xsl:variable name="myTetras"
                    select="for $value in distinct-values(for $each in distinct-values((@ism:ownerProducer | @ism:releasableTo | @ism:displayOnlyTo | @ism:FGIsourceOpen | @ism:FGIsourceProtected)) return util:tokenize($each)) return if ($catt//catt:Tetragraph[catt:TetraToken=$value]) then $value else null"/>
      <xsl:variable name="tetrasWithReleasableTo"
                    select="distinct-values(for $value in $myTetras return        if($catt//catt:Tetragraph[catt:TetraToken=$value]/catt:TetraToken/@ism:releasableTo)      then $value        else null)"/>
      <xsl:variable name="moreRestrictiveTetras"
                    select="for $tetra in $tetrasWithReleasableTo return       if (every $value in $releasableToCountries satisfies index-of(distinct-values(util:tokenize(util:getTetragraphReleasability($tetra))),$value)&gt;0)        then null else $tetra"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="empty($moreRestrictiveTetras)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="empty($moreRestrictiveTetras)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00376][Error] A portion using tetragraphs may not have a releasableTo 
		    	that is less restrictive than the releasability of any tetragraph or organization tokens used
		    	in the same portion’s releasableTo, displayOnlyTo, FGIsourceOpen, or FGIsourceProtected attributes.
		    	If a tetragraph XXXX in any of the attributes ownerProducer, releasableTo, displayOnlyTo, FGIsourceOpen, 
		    	or FGIsourceProtected is itself marked as ism:releasableTo in the Tetragraph Taxonomy, then see if all
		    	the countries that the portion is releasableTo are also countries that the tetragraph XXXX is releasableTo.  If not, error. 
		    	The following tetragraphs have a more restrictive releasability than the portion: 
		    	<xsl:text/>
                  <xsl:value-of select="string-join($moreRestrictiveTetras,', ')"/>
                  <xsl:text/>
		             </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="exists($catt//catt:Tetragraphs)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="exists($catt//catt:Tetragraphs)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>ISMCAT Taxonomy does not exist!</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M594"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M594"/>
   <xsl:template match="@*|node()" priority="-2" mode="M594">
      <xsl:apply-templates select="*" mode="M594"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00377-->


	<!--RULE ValidateTokenValuesExistenceInList-R1-->
<xsl:template match="*[@ism:ownerProducer and  @ism:joint=true()]"
                 priority="1000"
                 mode="M595">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:ownerProducer and  @ism:joint=true()]"
                       id="ValidateTokenValuesExistenceInList-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $searchTerm in tokenize(normalize-space(string(normalize-space(string-join(util:decomposeTetragraphs(string(./@ism:ownerProducer)),' ')))), ' ') satisfies                    $searchTerm = tokenize(normalize-space(string-join(util:decomposeTetragraphs(string(@ism:releasableTo)),' ')), ' ') or (some $Term in tokenize(normalize-space(string-join(util:decomposeTetragraphs(string(@ism:releasableTo)),' ')), ' ') satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $searchTerm in tokenize(normalize-space(string(normalize-space(string-join(util:decomposeTetragraphs(string(./@ism:ownerProducer)),' ')))), ' ') satisfies $searchTerm = tokenize(normalize-space(string-join(util:decomposeTetragraphs(string(@ism:releasableTo)),' ')), ' ') or (some $Term in tokenize(normalize-space(string-join(util:decomposeTetragraphs(string(@ism:releasableTo)),' ')), ' ') satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'         [ISM-ID-00377][Error] All @ism:ownerProducer values in a JOINT document must be in the ism:releasableTo attribute'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M595"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M595"/>
   <xsl:template match="@*|node()" priority="-2" mode="M595">
      <xsl:apply-templates select="*" mode="M595"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00382-->


	<!--RULE -->
<xsl:template match="*[count(tokenize(normalize-space(string(@ism:ownerProducer)), ' ')) = 1]"
                 priority="1000"
                 mode="M598">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[count(tokenize(normalize-space(string(@ism:ownerProducer)), ' ')) = 1]"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:joint=true())"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="not(@ism:joint=true())">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[ISM-ID-00382][Error] For all elements with single-valued @ism:ownerProducer, @ism:joint must NOT be true.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M598"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M598"/>
   <xsl:template match="@*|node()" priority="-2" mode="M598">
      <xsl:apply-templates select="*" mode="M598"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00383-->


	<!--RULE -->
<xsl:template match="*[@ism:joint=true()]" priority="1000" mode="M599">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl" context="*[@ism:joint=true()]"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:ownerProducer, ('USA'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:ownerProducer, ('USA'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[ISM-ID-00383][Error] For elements with @ism:joint set to true, one of the values of @ism:ownerProducer must be USA.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M599"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M599"/>
   <xsl:template match="@*|node()" priority="-2" mode="M599">
      <xsl:apply-templates select="*" mode="M599"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00453-->


	<!--RULE ISM-ID-00453-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^HCS-P-[A-Z0-9]{1,6}$'))]"
                 priority="1000"
                 mode="M608">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^HCS-P-[A-Z0-9]{1,6}$'))]"
                       id="ISM-ID-00453-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:classification, ('TS'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:classification, ('TS'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00453][Error] If ISM_USGOV_RESOURCE and attribute SCIcontrols contains the name token [HCS-P-XXXXXX], 
            where X is represented by the regular expression character class [A-Z0-9]{1,6},
            then attribute classification must have a value of [TS].
            
            Human Readable: A USA document with HCS-PRODUCT subcompartment data must be classified TOP SECRET.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M608"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M608"/>
   <xsl:template match="@*|node()" priority="-2" mode="M608">
      <xsl:apply-templates select="*" mode="M608"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00511-->


	<!--RULE ISM-ID-00511-R1-->
<xsl:template match="//arh:Security" priority="1000" mode="M610">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="//arh:Security"
                       id="ISM-ID-00511-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:resourceElement = true()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="@ism:resourceElement = true()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00511][Error] arh:Security/@ism:resourceElement attribute must be true.
            
            Human Readable: arh:Security element must contain @ism:resourceElement attribute and @ism:resourceElement
            must equal 'true'.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M610"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M610"/>
   <xsl:template match="@*|node()" priority="-2" mode="M610">
      <xsl:apply-templates select="*" mode="M610"/>
   </xsl:template>
</xsl:stylesheet>
<!--UNCLASSIFIED-->
