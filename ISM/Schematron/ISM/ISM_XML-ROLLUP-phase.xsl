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
         <xsl:attribute name="phase">ROLLUP</xsl:attribute>
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
            <xsl:attribute name="id">ISM-ID-00064</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00064</xsl:attribute>
            <svrl:text>
        [ISM-ID-00064][Error] If ISM_USGOV_RESOURCE and any element meeting
        ISM_CONTRIBUTES in the document have the attribute @ism:FGIsourceOpen containing any value then
        the ISM_RESOURCE_ELEMENT must have either @ism:FGIsourceOpen or @ism:FGIsourceProtected with a value.
        
        Human Readable: USA documents having FGI Open data must have FGI Open or FGI Protected at
        the resource level. 
    </svrl:text>
            <svrl:text>
        If IC Markings System Register and Manual marking rules do not apply to the document then this
        rule does not apply and this rule returns true. If the current element has attribute @ism:FGIsourceOpen
        specified and does not have attribute @ism:excludeFromRollup set to true, this rule ensures that
        the resourceElement has one of the following attributes specified: @ism:FGIsourceOpen or @ism:FGIsourceProtected.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M276"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00065</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00065</xsl:attribute>
            <svrl:text>
        [ISM-ID-00065][Error] If ISM_USGOV_RESOURCE and any element meeting ISM_CONTRIBUTES in the document 
        have the attribute @ism:FGIsourceProtected containing any value then the ISM_RESOURCE_ELEMENT 
        must have @ism:FGIsourceProtected with a value.
        
        Human Readable: USA documents having FGI Protected data must have FGI Protected at the resource level.
    </svrl:text>
            <svrl:text>
        If IC Markings System Register and Manual rules do not apply to the document then the rule does not apply
        and this rule returns true. If any element has attribute @ism:FGIsourceProtected specified 
        with a non-empty value and does not have attribute @ism:excludeFromRollup set to true, 
        then this rule ensures that the banner has attribute @ism:FGIsourceProtected specified with 
        a non-empty value.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M277"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00066</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00066</xsl:attribute>
            <svrl:text>
        [ISM-ID-00066][Error] If ISM_USGOV_RESOURCE and: 
        1. Any element meeting ISM_CONTRIBUTES in the document has the attribute @ism:disseminationControls containing [FOUO]
        AND
        2. ISM_RESOURCE_ELEMENT has the attribute @ism:classification [U]
        AND
        3. No element meeting ISM_CONTRIBUTES in the document has @ism:nonICmarkings
        AND
        4. Elements meeting ISM_CONTRIBUTES only contain dissemination controls 
        [REL], [RELIDO],[NF], [DISPLAYONLY], [EYES], and [FOUO].
        
        Then the ISM_RESOURCE_ELEMENT must have @ism:disseminationControls containing [FOUO].
        
        Human Readable: USA Unclassified documents having FOUO data, no non IC Markings, and only 
        contains dissemination controls [REL], [RELIDO], [NF], [DISPLAYONLY], [EYES], and [FOUO] must have 
        FOUO at the resource level.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, the current element is the ISM_RESOURCE_ELEMENT,
        some element meeting ISM_CONTRIBUTES specifies attribute @ism:disseminationControls
        with a value containing [FOUO], the ISM_RESOURCE_ELEMENT specifies the attribute
        @ism:classification with a value of [U], no element meeting ISM_CONTRIBUTES
        specifies attribute @ism:nonICmarkings, and elements meeting ISM_CONTRIBUTES
        only contain @ism:disseminationControls with tokens [REL], [RELIDO], [NF], [DISPLAYONLY], [EYES], and [FOUO], 
        then the resource element must contain @ism:disseminationControls with a value containing the token [FOUO].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M278"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00067</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00067</xsl:attribute>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE and an element meeting
    ISM_CONTRIBUTES specifies attribute @ism:disseminationControls with a value containing the token
    [OC], then this rule ensures that the ISM_RESOURCE_ELEMENT specifies the attribute
    @ism:disseminationControls with a value containing the token [OC]. 
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M279"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00068</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00068</xsl:attribute>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE and an element meeting
    ISM_CONTRIBUTES specifies attribute @ism:disseminationControls with a value containing the token
    [IMC], then this rule ensures that the ISM_RESOURCE_ELEMENT specifies the attribute
    @ism:disseminationControls with a value containing the token [IMC]. 
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M280"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00070</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00070</xsl:attribute>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE and an element meeting
    ISM_CONTRIBUTES specifies attribute @ism:disseminationControls with a value containing the token
    [NF], then this rule ensures that the ISM_RESOURCE_ELEMENT specifies the attribute
    @ism:disseminationControls with a value containing the token [NF]. 
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M281"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00071</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00071</xsl:attribute>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE and an element meeting
    ISM_CONTRIBUTES specifies attribute @ism:disseminationControls with a value containing the token
    [PR], then this rule ensures that the ISM_RESOURCE_ELEMENT specifies the attribute
    @ism:disseminationControls with a value containing the token [PR]. 
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M282"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00072</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00072</xsl:attribute>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE and an element meeting
    ISM_CONTRIBUTES specifies attribute @ism:atomicEnergyMarkings with a value containing the token
    [RD], then this rule ensures that the ISM_RESOURCE_ELEMENT specifies the attribute
    @ism:atomicEnergyMarkings with a value containing the token [RD]. 
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M283"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00073</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00073</xsl:attribute>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE and an element meeting
    ISM_CONTRIBUTES specifies attribute @ism:atomicEnergyMarkings with a value containing the token
    [RD-CNWDI], then this rule ensures that the ISM_RESOURCE_ELEMENT specifies the attribute
    @ism:atomicEnergyMarkings with a value containing the token [RD-CNWDI]. 
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M284"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00074</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00074</xsl:attribute>
            <svrl:text>
        [ISM-ID-00074][Error] If ISM_USGOV_RESOURCE and any element meeting ISM_CONTRIBUTES 
        in the document has the attribute @ism:atomicEnergyMarkings containing [RD-SG-##] 
        then the ISM_RESOURCE_ELEMENT must have @ism:atomicEnergyMarkings containing [RD-SG-##]. 
        ## represent digits 1 through 99 the ## must match.
        
        Human Readable: USA documents having Restricted SIGMA-## Data must have the same Restricted SIGMA-## Data 
        at the resource level.
    </svrl:text>
            <svrl:text>
        If IC Markings System Register and Manual rules do not apply to the document then the rule does not apply
        and this rule returns true. This rule ensures that no element that does not have attribute @ism:excludeFromRollup 
        set to true has attribute @ism:atomicEnergyMarkings specified
        with a value containing [RD-SG-##], where ## is represented by a regular expression matching
        numbers 1 through 99, unless the resourceElement also has attribute
        @ism:atomicEnergyMarkings specified with a value containing [RD-SG-##].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M285"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00075</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00075</xsl:attribute>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE and an element meeting ISM_CONTRIBUTES
    specifies attribute @ism:atomicEnergyMarkings with a value containing the token
    [FRD] and the exception value(s) are not present, then this rule ensures that 
    the ISM_RESOURCE_ELEMENT specifies the attribute @ism:atomicEnergyMarkings with a 
    value containing the token [FRD].
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M286"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00077</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00077</xsl:attribute>
            <svrl:text>
        [ISM-ID-00077][Error] If ISM_USGOV_RESOURCE and any element meeting ISM_CONTRIBUTES in the 
        document has the attribute @ism:atomicEnergyMarkings containing [FRD-SG-##] and the ISM_RESOURCE_ELEMENT
        does not have @ism:atomicEnergyMarkings containing [RD], then the ISM_RESOURCE_ELEMENT must have 
        @ism:atomicEnergyMarkings containing [FRD-SG-##]. ## represent digits 1 through 99 the ## must match.
        
        Human Readable: USA documents having Formerly Restricted SIGMA-## data and not having RD data 
        must have the same Formerly Restricted SIGMA-## Data at the resource level.
    </svrl:text>
            <svrl:text>
        If IC Markings System Register and Manual rules do not apply to the document then the rule does not apply
        and this rule returns true. This rule ensures that no element that does not have attribute @ism:excludeFromRollup 
        set to true has attribute @ism:atomicEnergyMarkings specified with a value containing [FRD-SG-##], 
        where ## is represented by a regular expression matching numbers 1 through 99, unless the resourceElement 
        also has attribute @ism:atomicEnergyMarkings specified with a value containing [FRD-SG-##] or [RD] is specified 
        on the ISM_RESOURCE_ELEMENT.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M287"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00078</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00078</xsl:attribute>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE, and the ISM_RESOURCE_ELEMENT 
    specifies attribute @ism:classification with a value of 
    U an element meeting ISM_CONTRIBUTES
    specifies attribute @ism:atomicEnergyMarkings with a value containing the token
    [DCNI], then this rule ensures that the ISM_RESOURCE_ELEMENT specifies the 
    attribute @ism:atomicEnergyMarkings with a value containing the token [DCNI].
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M288"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00079</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00079</xsl:attribute>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE, and the ISM_RESOURCE_ELEMENT 
    specifies attribute @ism:classification with a value of 
    U an element meeting ISM_CONTRIBUTES
    specifies attribute @ism:atomicEnergyMarkings with a value containing the token
    [UCNI], then this rule ensures that the ISM_RESOURCE_ELEMENT specifies the 
    attribute @ism:atomicEnergyMarkings with a value containing the token [UCNI].
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M289"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00080</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00080</xsl:attribute>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE and an element meeting
    ISM_CONTRIBUTES specifies attribute @ism:disseminationControls with a value containing the token
    [DSEN], then this rule ensures that the ISM_RESOURCE_ELEMENT specifies the attribute
    @ism:disseminationControls with a value containing the token [DSEN]. 
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M290"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00081</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00081</xsl:attribute>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE and an element meeting
    ISM_CONTRIBUTES specifies attribute @ism:disseminationControls with a value containing the token
    [FISA], then this rule ensures that the ISM_RESOURCE_ELEMENT specifies the attribute
    @ism:disseminationControls with a value containing the token [FISA]. 
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M291"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00084</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00084</xsl:attribute>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE, and the ISM_RESOURCE_ELEMENT 
    specifies attribute @ism:classification with a value of 
    U an element meeting ISM_CONTRIBUTES
    specifies attribute @ism:nonICmarkings with a value containing the token
    [DS], then this rule ensures that the ISM_RESOURCE_ELEMENT specifies the 
    attribute @ism:nonICmarkings with a value containing the token [DS].
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M292"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00085</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00085</xsl:attribute>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE and an element meeting ISM_CONTRIBUTES
    specifies attribute @ism:nonICmarkings with a value containing the token
    [XD] and the exception value(s) are not present, then this rule ensures that 
    the ISM_RESOURCE_ELEMENT specifies the attribute @ism:nonICmarkings with a 
    value containing the token [XD].
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M293"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00086</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00086</xsl:attribute>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE and an element meeting
    ISM_CONTRIBUTES specifies attribute @ism:nonICmarkings with a value containing the token
    [ND], then this rule ensures that the ISM_RESOURCE_ELEMENT specifies the attribute
    @ism:nonICmarkings with a value containing the token [ND]. 
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M294"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00087</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00087</xsl:attribute>
            <svrl:text>
        [ISM-ID-00087][Error] Classified USA documents having SBU-NF Data must have NF at the resource level.
    </svrl:text>
            <svrl:text>
        If IC Markings System Register and Manual rules do not apply to the
        document then the rule does not apply and the rule returns true. If any element has
        attribute @ism:nonICmarkings specified with a value containing [SBU-NF], does not have attribute
        @ism:excludeFromRollup set to true, and the resourceElement has attribute @ism:classification
        specified with a value other than [U], this rule ensures that the resourceElement has
        attribute @ism:disseminationControls specified with a value containing [NF]. 
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M295"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00088</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00088</xsl:attribute>
            <svrl:text>
        [ISM-ID-00088][Error] If ISM_USGOV_RESOURCE and @ism:releasableTo is specified on the resource
        element then all classified portions must specify @ism:releasableTo and all Unclass portions must be REL or contain
        no caveats. 
        
        Human Readable: USA documents having any classified portion that is not Releasable or having
        unclassified portions with disseminationControls that are not [REL] cannot be REL at the resource level.
    </svrl:text>
            <svrl:text>
        If IC Markings System Register and Manual rules apply to the document, this rule verifies
        that all portions either have the attribute @ism:classification specified with a value of [U] and uncaveated or REL
        or classified portions of the document have the attribute @ism:releasableTo. Attribute @ism:releasableTo is only valid on
        an element if attribute @ism:disseminationControls is specified with a value containing [REL] or [EYES], as [REL]
        supersedes [EYES] in the banner. If any elements do not meet either of the two requirements stated above, then
        the assertion fails since attribute @ism:releasableTo appears on the banner but is not present on all classified
        portions.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M296"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00090</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00090</xsl:attribute>
            <svrl:text>
        [ISM-ID-00090][Error] If ISM_USGOV_RESOURCE and any element: 
        1. Meets ISM_CONTRIBUTES
        AND
        2. Has the attribute @ism:disseminationControls containing [REL]
        Then the ISM_RESOURCE_ELEMENT must not have attribute @ism:disseminationControls containing [EYES]. 
        
        Human Readable: USA documents with any portion that is REL must not be EYES at the resource level.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_CAPO_RESOURCE, the current element is the 
        ISM_RESOURCE_ELEMENT, and some element meeting ISM_CONTRIBUTES specifies
        attribute @ism:disseminationControls with a value containing [REL], 
        this rule ensures that ISM_RESOURCE_ELEMENT does not specify attribute
        @ism:disseminationControls or specifies the attribute with a value
        that does not contain the token [EYES].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M297"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00104</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00104</xsl:attribute>
            <svrl:text>
    [ISM-ID-00104][Error] If the document is an ISM_USGOV_RESOURCE and any
    element in the document is: 
      1. Unclassified and meets ISM_CONTRIBUTES 
        AND 
      2. Has the attribute @ism:nonICmarkings containing [SBU-NF] 
        AND
      3. The ISM_RESOURCE_ELEMENT has attribute @ism:nonICmarkings does not contain [XD] or [ND] 
        AND
      4. The ISM_RESOURCE_ELEMENT has attribute @ism:disseminationControls does not contain [NF]
    Then the ISM_RESOURCE_ELEMENT must have @ism:nonICmarkings containing [SBU-NF]. 
    
    Human Readable: USA Unclassified documents having SBU-NF and not having XD, ND, or explicit Foreign Disclosure and
    Release markings must have SBU-NF at the resource level.
  </svrl:text>
            <svrl:text>
    If the document is Unclassified and is an ISM_USGOV_RESOURCE, the current
    element is the ISM_RESOURCE_ELEMENT, some element meeting ISM_CONTIBUTES specifies attribute
    @ism:nonICmarkings with a value containing the token [SBU-NF], and the attribute @ism:nonICmarkings
    on the ISM_RESOURCE_ELEMENT does not contain the token [XD] or [ND], and the attribute 
    @ism:disseminationControls on the resource element does not contain the token [NF]; 
    this rule ensures sure that ISM_RESOURCE_ELEMENT specifies 
    attribute @ism:nonICmarkings with a value containing the token [SBU-NF].</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M300"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00105</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00105</xsl:attribute>
            <svrl:text>
    [ISM-ID-00105][Error] If the document is an ISM_USGOV_RESOURCE and any
    element in the document is: 
    1. Unclassifed and meets ISM_CONTRIBUTES 
      AND 
    2. Has the attribute @ism:nonICmarkings containing [SBU] 
      AND 
    3. No element meeting ISM_CONTRIBUTES in the document has @ism:nonICmarkings containing any of [SBU-NF], 
       [XD], or [ND]
    Then the ISM_RESOURCE_ELEMENT must have @ism:nonICmarkings containing [SBU]. 
    
    Human Readable: USA Unclassified documents having SBU and not having SBU-NF, XD, or ND must have SBU at the resource level. 
  </svrl:text>
            <svrl:text>
    If the document is Unclassified and is an ISM_USGOV_RESOURCE, the current
    element is the ISM_RESOURCE_ELEMENT, some element meeting ISM_CONTIBUTES specifies attribute
    @ism:nonICmarkings with a value containing the token [SBU], and no element meeting
    ISM_CONTRIBUTES specifies attribute @ism:nonICmarkings with a value containing the token
    [SBU-NF], [XD], and [ND], then this rule ensures that ISM_RESOURCE_ELEMENT sepcifies attribute
    @ism:nonICmarkings with a value containing the token [SBU]. 
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M301"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00145</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00145</xsl:attribute>
            <svrl:text>
        [ISM-ID-00145][Error] If ISM_USGOV_RESOURCE and any element in the document: 
        1. Meets ISM_CONTRIBUTES
        AND
        2. Has the attribute @ism:nonICmarkings containing [LES]
        AND
        3. No element meeting ISM_CONTRIBUTES in the document has @ism:nonICmarkings containing any of [LES-NF]
        Then the ISM_RESOURCE_ELEMENT must have @ism:nonICmarkings containing [LES].
        
        Human Readable: USA documents having LES and not having LES-NF must have LES at the resource level.
    </svrl:text>
            <svrl:text>
      If the document is an ISM_USGOV_RESOURCE, the current element is the 
      ISM_RESOURCE_ELEMENT, some element meeting ISM_CONTIBUTES specifies
      attribute @ism:nonICmarkings with a value containing the token [LES], and
      no element meeting ISM_CONTRIBUTES specifies attribute @ism:nonICmarkings
      with a value containing the token [LES-NF], then this rule ensures that
      ISM_RESOURCE_ELEMENT sepcifies attribute @ism:nonICmarkings with a value
      containing the token [LES].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M321"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00146</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00146</xsl:attribute>
            <svrl:text>
        [ISM-ID-00146][Error] If ISM_USGOV_RESOURCE and there exist at least 2 elements in the document:
        1. Each element: Meets ISM_CONTRIBUTES
        AND
        2. One of the elements: Has the attribute @ism:nonICmarkings containing [LES-NF]
        AND
        3. One of the elements: meets ISM_CONTRIBUTES_CLASSIFIED
        Then the ISM_RESOURCE_ELEMENT must have @ism:disseminationControls containing [NF].
        
        Human Readable: Classified USA documents having LES-NF Data must have NF at the resource level.
    </svrl:text>
            <svrl:text>
        If IC Markings System Register and Manual rules do not apply to the document then the rule does not apply
        and this rule returns true. If any element has attribute @ism:nonICmarkings specified 
        with a value containing [LES-NF] and the resourceElement has attribute @ism:classification specified 
        with a value other than [U], then this rule ensures that the resourceElement has attribute 
        @ism:disseminationControls specified with a value containing [NF].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M322"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00147</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00147</xsl:attribute>
            <svrl:text>
        [ISM-ID-00147][Error] If ISM_USGOV_RESOURCE and there exist at least 2 elements in the document:
        1. Each element: Meets ISM_CONTRIBUTES
        AND
        2. One of the elements: Has the attribute @ism:nonICmarkings containing [LES-NF]
        AND
        3. One of the elements: meets ISM_CONTRIBUTES_CLASSIFIED
        Then the ISM_RESOURCE_ELEMENT must have @ism:nonICmarkings containing [LES].
        
        Human Readable: Classified USA documents having LES-NF Data must have LES at the resource level.
    </svrl:text>
            <svrl:text>
        If IC Markings System Register and Manual rules do not apply to the document then the rule does not apply
        and this rule returns true. If any element has attribute @ism:nonICmarkings specified 
        with a value containing [LES-NF] and the resourceElement has attribute @ism:classification specified 
        with a value other than [U], then this rule ensures that the resourceElement has attribute @ism:nonICmarkings
        specified with a value containing [LES].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M323"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00149</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00149</xsl:attribute>
            <svrl:text>
    [ISM-ID-00149][Error] If the document is an ISM_USGOV_RESOURCE and:
    1. Any element in the document meets ISM_CONTRIBUTES in the document has the attribute @ism:nonICmarkings
       contain [LES-NF] 
      AND 
    2. ISM_RESOURCE_ELEMENT has the attribute @ism:classification [U] 
      AND 
    3. ISM_RESOURCE_ELEMENT does not have the attribute @ism:disseminationControls [NF] 
       THEN the ISM_RESOURCE_ELEMENT must have @ism:nonICmarkings containing [LES-NF]
    
    Human Readable: Unclassified USA documents having LES-NF and not having NF 
    must have LES-NF at the resource level.
  </svrl:text>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE, the current element is the
    ISM_RESOURCE_ELEMENT, some element meeting ISM_CONTRIBUTES specifies attribute @ism:nonICmarkings
    with a value containing the token [LES-NF], and the ISM_RESOURCE_ELEMENT does not have
    attribute @ism:disseminationControls with a value containing the token [NF]; then this rule 
    ensures that ISM_RESOURCE_ELEMENT specifies attribute @ism:nonICmarkings with a value containing 
    the token [LES-NF].
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M325"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00165</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00165</xsl:attribute>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE and an element meeting
    ISM_CONTRIBUTES specifies attribute @ism:disseminationControls with a value containing the token
    [RS], then this rule ensures that the ISM_RESOURCE_ELEMENT specifies the attribute
    @ism:disseminationControls with a value containing the token [RS]. 
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M333"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00176</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00176</xsl:attribute>
            <svrl:text>
        [ISM-ID-00176][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:atomicEnergyMarkings has a name token containing [RD] or [FRD], 
        then attributes @ism:declassDate and @ism:declassEvent cannot be specified
        on the resourceElement.

        Human Readable: Automatic declassification of documents containing 
        RD or FRD information is prohibited. Attributes declassDate and 
        declassEvent cannot be used in the classification authority block when 
        RD or FRD is present.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE, for each element which 
    	has attribute ism:atomicEnergyMarkings specified with a value containing
        the token [RD] or [FRD], this rule ensures that the resourceElement does not
    	have attributes ism:declassDate or ism:declassEvent specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M341"/>
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
            <xsl:attribute name="id">ISM-ID-00298</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00298</xsl:attribute>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE and an element meeting ISM_CONTRIBUTES
    specifies attribute @ism:atomicEnergyMarkings with a value containing the token
    [TFNI] and the exception value(s) are not present, then this rule ensures that 
    the ISM_RESOURCE_ELEMENT specifies the attribute @ism:atomicEnergyMarkings with a 
    value containing the token [TFNI].
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M433"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00315</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00315</xsl:attribute>
            <svrl:text>
        [ISM-ID-00315][Error] If classified element meets ISM_CONTRIBUTES and
        attribute @ism:ownerProducer contains the token [NATO], then attribute @ism:declassException must be
        specified with a value of [NATO] or [NATO-AEA] on the resourceElement. 
        
        Human Readable: Any document with non-resource classified elements that contributes to the document's banner 
        roll-up and has NATO Information must also specify a NATO declass exemption on the banner. 
    </svrl:text>
            <svrl:text>
        In a classified document that meets ISM_USGOV_RESOURCE, for each
        element which is not the $ISM_RESOURCE_ELEMENT and meets ISM_CONTRIBUTES and specifies
        attribute @ism:ownerProducer with a value containing the token [NATO], this rule ensures that
        attribute @ism:declassExemption on the resource element is specified with a value containing
        the token [NATO] or [NATO-AEA]. 
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M439"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00318</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00318</xsl:attribute>
            <svrl:text>
    Where an element is the resource element and contains either the @ism:releasableTo or 
    @ism:displayOnlyTo attributes, check that the values specified meet minimum roll-up conditions. 
    Check all contributing portions against the banner for the existence of common countries 
    ensuring that the countries in the banner are the intersection of all contributing portions. 
    Any tetragraphs whose decomposable flag is true will be decomposed into their representative countries.
    
    Once the minimum possibility of intersecting countries is determined, the rule checks that  
    all portions of the banner are included in the subset.  The rule then checks for the case where 
    there are no common countries to be rolled up to the resource element.  Finally, the rule checks to
    ensure that if the banner countries are a subset of the common countries, that a
    compilationReason is specified.  If a compilationReason is not specified, then the banner
    displayOnlyTo countries must be the set of common countries from all contributing portions.
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M442"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00320</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00320</xsl:attribute>
            <svrl:text>
    Where an element is the resource element and contains either the @ism:releasableTo or 
    @ism:displayOnlyTo attributes, check that the values specified meet minimum roll-up conditions. 
    Check all contributing portions against the banner for the existence of common countries 
    ensuring that the countries in the banner are the intersection of all contributing portions. 
    Any tetragraphs whose decomposable flag is true will be decomposed into their representative countries.
    
    Once the minimum possibility of intersecting countries is determined, the rule checks that  
    all portions of the banner are included in the subset.  The rule then checks for the case where 
    there are no common countries to be rolled up to the resource element.  Finally, the rule checks to
    ensure that if the banner countries are a subset of the common countries, that a
    compilationReason is specified.  If a compilationReason is not specified, then the banner
    displayOnlyTo countries must be the set of common countries from all contributing portions.
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M444"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00343</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00343</xsl:attribute>
            <svrl:text>
        [ISM-ID-00343][Error] If ISM_USGOV_RESOURCE and there exists a token in @ism:SCIcontrols for portions that contribute to
        rollup, then they must also be specified in the @ism:SCIcontrols attribute on the ISM_RESOURCE_ELEMENT.
        
        Human Readable: All SCI controls specified in the document that contribute to rollup must
        be rolled up to the resource level.
    </svrl:text>
            <svrl:text>
       If the document is an ISM_USGOV_RESOURCE match on the ISM_RESOURCE_ELEMENT if there are any @ism:SCIcontrols 
       values specified on portions that are not @ism:excludeFromRollup="true" and then ensure that all the tokens found exist on the
       element are matched to. If there are any tokens not present in our element that exist elsewhere
       in the document's contributing portions, store them in the missingSCI variable. Then this rule ensures
       that the missingSCI variable is empty or return an error message that specifies which tokens 
       are missing.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M456"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00347</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00347</xsl:attribute>
            <svrl:text>
        [ISM-ID-00347][Error] If ISM_USGOV_RESOURCE and if there exists a token in @ism:SARIdentifier for portions that contribute to
        rollup then they must also be specified in the @ism:SARIdentifier attribute on the ISM_RESOURCE_ELEMENT.
        
        Human Readable: All SAR Identifiers specified in the document that contribute to rollup must
        be rolled up to the resource level.
    </svrl:text>
            <svrl:text>
       If ISM_USGOV_RESOURCE, match on the ISM_RESOURCE_ELEMENT if there are any @ism:SARIdentifier values specified on portions
       that are not @ism:excludeFromRollup="true" and then ensure that all the tokens found exist on the
       element are matched to. If there are any tokens not present in our element that exist elsewhere
       in the document's contributing portions, store them in the missingSAR variable. Then check
       that the missingSAR variable is empty or return an error message that specifies which tokens 
       are missing.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M460"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00373</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00373</xsl:attribute>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE and an element meeting
    ISM_CONTRIBUTES specifies attribute @ism:nonICmarkings with a value containing the token
    [SSI], then this rule ensures that the ISM_RESOURCE_ELEMENT specifies the attribute
    @ism:nonICmarkings with a value containing the token [SSI]. 
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M482"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00389</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00389</xsl:attribute>
            <svrl:text>
    If the document is an ISM_USGOV_RESOURCE and an element meeting
    ISM_CONTRIBUTES specifies attribute @ism:disseminationControls with a value containing the token
    [RAWFISA], then this rule ensures that the ISM_RESOURCE_ELEMENT specifies the attribute
    @ism:disseminationControls with a value containing the token [RAWFISA]. 
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M491"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00461</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00461</xsl:attribute>
            <svrl:text> 
    [ISM-ID-00461][Error] If ISM_USDOD_RESOURCE and 
    1. not ISM_DOD_DISTRO_EXEMPT
    AND 
    2. Attribute @ism:noticeType of any portion that is not @ism:excludeFromRollup="true" contains [ITAR-EAR],
    then there must be @ism:noticeType=[ITAR-EAR] on the resource element. 
    
    Human Readable: All US DOD documents that do not claim exemption from DoD5230.24 and that have an [ITAR-EAR] notice
    on any portion must have an [ITAR-EAR] notice on the resource element. 
  </svrl:text>
            <svrl:text>
    If the document is an ISM_USDOD_RESOURCE and not ISM_DOD_DISTRO_EXEMPT and has any portion with @ism:noticeType=[ITAR-EAR], and
    the current element is the ISM_RESOURCE_ELEMENT, this rule ensures that attribute @ism:noticeType is
    specified on the resource element with a value of [ITAR-EAR].
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M505"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00502</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00502</xsl:attribute>
            <svrl:text>
        [ISM-ID-00502][Error] If ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, and
        there exists a token in @ism:cuiBasic for portions that contribute to rollup, then all such tokens must
        also be specified in the @ism:cuiBasic attribute on the ISM_RESOURCE_ELEMENT. 
        
        Human Readable: All CUI Basic category markings specified in the document that contribute to
        rollup must be rolled up to the resource level. 
    </svrl:text>
            <svrl:text> If the document is an ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, match on
        the ISM_RESOURCE_ELEMENT. If there are any @ism:cuiBasic values specified on portions that
        are not @ism:excludeFromRollup="true", then ensure that all the tokens found exist on the matched resource
        element. If there are any tokens not present in the matched resource element that exist
        elsewhere in the document's contributing portions, store them in the missingCuiBasic variable.
        Then this rule ensures that the missingCuiBasic variable is empty or else return an error message that
        specifies which tokens are missing. 
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M542"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00503</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00503</xsl:attribute>
            <svrl:text> 
        [ISM-ID-00503][Error] If ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, and
        there exists a token in @ism:cuiSpecified for portions that contribute to rollup, then all such tokens must
        also be specified in the @ism:cuiSpecified attribute on the ISM_RESOURCE_ELEMENT. 
        
        Human Readable: All CUI Specified category markings contained in the document that contribute to
        rollup must be rolled up to the resource level. 
    </svrl:text>
            <svrl:text> If the document is an ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, match on
        the ISM_RESOURCE_ELEMENT. If there are any @ism:cuiSpecified values in portions that
        are not @ism:excludeFromRollup="true", then ensure that all the tokens found exist on the matched resource
        element. If there are any tokens not present in the matched resource element that exist
        elsewhere in the document's contributing portions, store them in the missingCuiSpecified variable.
        Then this rule ensures that the missingCuiSpecified variable is empty or else return an error message that
        specifies which tokens are missing.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M543"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00521</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00521</xsl:attribute>
            <svrl:text>
        [ISM-ID-00521][Error] If ISM_USGOV_RESOURCE and any element: 
        1. Meets ISM_CONTRIBUTES
        AND
        2. Has the attribute @ism:disseminationControls containing [REL]
        Then the ISM_RESOURCE_ELEMENT MUST have attribute @ism:disseminationControls containing either [REL], [DISPLAYONLY] or [NF]. 
        
        Human Readable: USA documents with any portion that is REL must be one of REL, DISPLAYONLY or NF at the resource level.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_CAPCO_RESOURCE, and some element meeting ISM_CONTRIBUTES specifies
        attribute @ism:disseminationControls with a value containing [REL], 
        this rule ensures that ISM_RESOURCE_ELEMENT specifies attribute
        @ism:disseminationControls containing either the token [REL], [DISPLAYONLY] or [NF].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M556"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00528</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00528</xsl:attribute>
            <svrl:text> [ISM-ID-00528][Error] If ISM_USGOV_RESOURCE and if
        @ism:disseminationControls contains the token [EXEMPT_FROM_ICD501_DISCOVERY] for portions
        that contribute to rollup then [EXEMPT_FROM_ICD501_DISCOVERY] must also be specified in the
        @ism:disseminationControls attribute on the ISM_RESOURCE_ELEMENT. Human Readable: If the
        token [EXEMPT_FROM_ICD501_DISCOVERY] is found in any @ism:disseminationControls in portions
        that contribute to rollup, then @disseminationControls=[EXEMPT_FROM_ICD501_DISCOVERY] must
        be rolled up to the resource level. </svrl:text>
            <svrl:text> If ISM_USGOV_RESOURCE, find the ISM_RESOURCE_ELEMENT and determine if
        there are any @ism:disseminationControls in portions that contribute to rollup. If there are
        any @ism:disseminationControls containing the token [EXEMPT_FROM_ICD501_DISCOVERY] in
        portions that are not @ism:excludeFromRollup="true", then ensure that the
        ISM_RESOURCE_ELEMENT has @ism:disseminationControls containing
        [EXEMPT_FROM_ICD501_DISCOVERY]. </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M562"/>
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

   <!--PATTERN ISM-ID-00064-->


	<!--RULE ISM-ID-00064-R1-->
<xsl:template match="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]"
                 priority="1000"
                 mode="M276">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]"
                       id="ISM-ID-00064-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="if(not($ISM_USGOV_RESOURCE)) then true() else if(not(empty($partFGIsourceOpen))) then ($bannerFGIsourceOpen or $bannerFGIsourceProtected) else true()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="if(not($ISM_USGOV_RESOURCE)) then true() else if(not(empty($partFGIsourceOpen))) then ($bannerFGIsourceOpen or $bannerFGIsourceProtected) else true()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00064][Error] If ISM_USGOV_RESOURCE and any element meeting
            ISM_CONTRIBUTES in the document have the attribute @ism:FGIsourceOpen containing any value then
            the ISM_RESOURCE_ELEMENT must have either @ism:FGIsourceOpen or @ism:FGIsourceProtected with a value.
            
            Human Readable: USA documents having FGI Open data must have FGI Open or FGI Protected at
            the resource level. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M276"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M276"/>
   <xsl:template match="@*|node()" priority="-2" mode="M276">
      <xsl:apply-templates select="*" mode="M276"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00065-->


	<!--RULE ISM-ID-00065-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(empty($partFGIsourceProtected))]"
                 priority="1000"
                 mode="M277">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(empty($partFGIsourceProtected))]"
                       id="ISM-ID-00065-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:FGIsourceProtected"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:FGIsourceProtected">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00065][Error] If ISM_USGOV_RESOURCE and any element meeting ISM_CONTRIBUTES in the document 
            have the attribute @ism:FGIsourceProtected containing any value then the ISM_RESOURCE_ELEMENT 
            must have @ism:FGIsourceProtected with a value.
            
            Human Readable: USA documents having FGI Protected data must have FGI Protected at the resource level.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M277"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M277"/>
   <xsl:template match="@*|node()" priority="-2" mode="M277">
      <xsl:apply-templates select="*" mode="M277"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00066-->


	<!--RULE ISM-ID-00066-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and index-of($dcTagsFound,'FOUO') &gt; 0 and util:containsAnyOfTheTokens(@ism:classification, ('U')) and count($partNonICmarkings_tok) = 0 and util:containsOnlyTheTokens(string-join($partDisseminationControls, ' '), ('REL', 'RELIDO', 'NF', 'EYES', 'DISPLAYONLY', 'FOUO'))]"
                 priority="1000"
                 mode="M278">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and index-of($dcTagsFound,'FOUO') &gt; 0 and util:containsAnyOfTheTokens(@ism:classification, ('U')) and count($partNonICmarkings_tok) = 0 and util:containsOnlyTheTokens(string-join($partDisseminationControls, ' '), ('REL', 'RELIDO', 'NF', 'EYES', 'DISPLAYONLY', 'FOUO'))]"
                       id="ISM-ID-00066-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('FOUO'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('FOUO'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00066][Error] If ISM_USGOV_RESOURCE and: 
            1. Any element meeting ISM_CONTRIBUTES in the document has the attribute @ism:disseminationControls containing [FOUO]
            AND
            2. ISM_RESOURCE_ELEMENT has the attribute @ism:classification [U]
            AND
            3. No element meeting ISM_CONTRIBUTES in the document has @ism:nonICmarkings
            AND
            4. Elements meeting ISM_CONTRIBUTES only contain dissemination controls 
            [REL], [RELIDO],[NF], [DISPLAYONLY], [EYES], and [FOUO].
            
            Then the ISM_RESOURCE_ELEMENT must have @ism:disseminationControls containing [FOUO].
            
            Human Readable: USA Unclassified documents having FOUO data, no non IC Markings, and only 
            contains dissemination controls [REL], [RELIDO], [NF], [DISPLAYONLY], [EYES], and [FOUO] must have 
            FOUO at the resource level.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M278"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M278"/>
   <xsl:template match="@*|node()" priority="-2" mode="M278">
      <xsl:apply-templates select="*" mode="M278"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00067-->


	<!--RULE AttributeContributesToRollup-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('OC')))]"
                 priority="1000"
                 mode="M279">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('OC')))]"
                       id="AttributeContributesToRollup-R1"/>

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
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00067][Error] USA documents having ORCON data must have ORCON at the resource level.'"/>
                  <xsl:text/> 
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M279"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M279"/>
   <xsl:template match="@*|node()" priority="-2" mode="M279">
      <xsl:apply-templates select="*" mode="M279"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00068-->


	<!--RULE AttributeContributesToRollup-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('IMC')))]"
                 priority="1000"
                 mode="M280">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('IMC')))]"
                       id="AttributeContributesToRollup-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('IMC'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('IMC'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00068][Error] USA documents having IMCON data must have IMCON at the resource level.'"/>
                  <xsl:text/> 
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M280"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M280"/>
   <xsl:template match="@*|node()" priority="-2" mode="M280">
      <xsl:apply-templates select="*" mode="M280"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00070-->


	<!--RULE AttributeContributesToRollup-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('NF')))]"
                 priority="1000"
                 mode="M281">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('NF')))]"
                       id="AttributeContributesToRollup-R1"/>

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
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00070][Error] USA documents having NF data must have NF at the resource level.'"/>
                  <xsl:text/> 
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M281"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M281"/>
   <xsl:template match="@*|node()" priority="-2" mode="M281">
      <xsl:apply-templates select="*" mode="M281"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00071-->


	<!--RULE AttributeContributesToRollup-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('PR')))]"
                 priority="1000"
                 mode="M282">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('PR')))]"
                       id="AttributeContributesToRollup-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('PR'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('PR'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00071][Error] USA documents having PROPIN data must have PROPIN at the resource level.'"/>
                  <xsl:text/> 
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M282"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M282"/>
   <xsl:template match="@*|node()" priority="-2" mode="M282">
      <xsl:apply-templates select="*" mode="M282"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00072-->


	<!--RULE AttributeContributesToRollup-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:atomicEnergyMarkings, ('RD')))]"
                 priority="1000"
                 mode="M283">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:atomicEnergyMarkings, ('RD')))]"
                       id="AttributeContributesToRollup-R1"/>

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
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00072][Error] USA documents having Restricted Data (RD) must have RD at the resource level.'"/>
                  <xsl:text/> 
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M283"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M283"/>
   <xsl:template match="@*|node()" priority="-2" mode="M283">
      <xsl:apply-templates select="*" mode="M283"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00073-->


	<!--RULE AttributeContributesToRollup-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:atomicEnergyMarkings, ('RD-CNWDI')))]"
                 priority="1000"
                 mode="M284">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:atomicEnergyMarkings, ('RD-CNWDI')))]"
                       id="AttributeContributesToRollup-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD-CNWDI'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD-CNWDI'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00073][Error] USA documents having Restricted CNWDI Data must have Restricted CNWDI Data at the resource level.'"/>
                  <xsl:text/> 
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M284"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M284"/>
   <xsl:template match="@*|node()" priority="-2" mode="M284">
      <xsl:apply-templates select="*" mode="M284"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00074-->


	<!--RULE ISM-ID-00074-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]"
                 priority="1000"
                 mode="M285">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]"
                       id="ISM-ID-00074-R1"/>
      <xsl:variable name="matchingTokens"
                    select="for $token in $partAtomicEnergyMarkings_tok return if(matches($token,'^RD-SG-[1-9][0-9]?$')) then $token else null"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $token in $matchingTokens satisfies index-of($bannerAtomicEnergyMarkings_tok, $token) &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $token in $matchingTokens satisfies index-of($bannerAtomicEnergyMarkings_tok, $token) &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00074][Error] If ISM_USGOV_RESOURCE and any element meeting ISM_CONTRIBUTES 
            in the document has the attribute @ism:atomicEnergyMarkings containing [RD-SG-##] 
            then the ISM_RESOURCE_ELEMENT must have @ism:atomicEnergyMarkings containing [RD-SG-##]. 
            ## represent digits 1 through 99 the ## must match.
            
            Human Readable: USA documents having Restricted SIGMA-## Data must have the same Restricted SIGMA-## Data 
            at the resource level.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M285"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M285"/>
   <xsl:template match="@*|node()" priority="-2" mode="M285">
      <xsl:apply-templates select="*" mode="M285"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00075-->


	<!--RULE AttributeContributesToRollupWithException-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:atomicEnergyMarkings, ('RD'))) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:atomicEnergyMarkings, ('FRD')))]"
                 priority="1000"
                 mode="M286">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:atomicEnergyMarkings, ('RD'))) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:atomicEnergyMarkings, ('FRD')))]"
                       id="AttributeContributesToRollupWithException-R1"/>

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
			               <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00075][Error] USA documents having Formerly Restricted Data (FRD) and not having Restricted Data (RD) must have FRD at the resource level.'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M286"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M286"/>
   <xsl:template match="@*|node()" priority="-2" mode="M286">
      <xsl:apply-templates select="*" mode="M286"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00077-->


	<!--RULE ISM-ID-00077-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings, ('RD')))]"
                 priority="1000"
                 mode="M287">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings, ('RD')))]"
                       id="ISM-ID-00077-R1"/>
      <xsl:variable name="matchingTokens"
                    select="for $token in $partAtomicEnergyMarkings_tok return if(matches($token,'^FRD-SG-[1-9][0-9]?$')) then $token else null"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $token in $matchingTokens satisfies index-of($bannerAtomicEnergyMarkings_tok, $token) &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $token in $matchingTokens satisfies index-of($bannerAtomicEnergyMarkings_tok, $token) &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
          [ISM-ID-00077][Error] If ISM_USGOV_RESOURCE and any element meeting ISM_CONTRIBUTES in the 
          document has the attribute @ism:atomicEnergyMarkings containing [FRD-SG-##] and the ISM_RESOURCE_ELEMENT
          does not have @ism:atomicEnergyMarkings containing [RD], then the ISM_RESOURCE_ELEMENT must have 
          @ism:atomicEnergyMarkings containing [FRD-SG-##]. ## represent digits 1 through 99 the ## must match.
          
          Human Readable: USA documents having Formerly Restricted SIGMA-## data and not having RD data 
          must have the same Formerly Restricted SIGMA-## Data at the resource level.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M287"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M287"/>
   <xsl:template match="@*|node()" priority="-2" mode="M287">
      <xsl:apply-templates select="*" mode="M287"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00078-->


	<!--RULE AttributeContributesToRollupWithClassification-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE      and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)      and util:containsAnyOfTheTokens(@ism:classification, ( 'U' ))     and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:atomicEnergyMarkings, ('DCNI')))]"
                 priority="1000"
                 mode="M288">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE      and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)      and util:containsAnyOfTheTokens(@ism:classification, ( 'U' ))     and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:atomicEnergyMarkings, ('DCNI')))]"
                       id="AttributeContributesToRollupWithClassification-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('DCNI'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('DCNI'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			               <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00078][Error] Unclassified USA documents having DCNI Data must have DCNI at the resource level.'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M288"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M288"/>
   <xsl:template match="@*|node()" priority="-2" mode="M288">
      <xsl:apply-templates select="*" mode="M288"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00079-->


	<!--RULE AttributeContributesToRollupWithClassification-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE      and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)      and util:containsAnyOfTheTokens(@ism:classification, ( 'U' ))     and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:atomicEnergyMarkings, ('UCNI')))]"
                 priority="1000"
                 mode="M289">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE      and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)      and util:containsAnyOfTheTokens(@ism:classification, ( 'U' ))     and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:atomicEnergyMarkings, ('UCNI')))]"
                       id="AttributeContributesToRollupWithClassification-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('UCNI'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('UCNI'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			               <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00079][Error] Unclassified USA documents having UCNI Data must have UCNI at the resource level.'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M289"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M289"/>
   <xsl:template match="@*|node()" priority="-2" mode="M289">
      <xsl:apply-templates select="*" mode="M289"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00080-->


	<!--RULE AttributeContributesToRollup-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('DSEN')))]"
                 priority="1000"
                 mode="M290">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('DSEN')))]"
                       id="AttributeContributesToRollup-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('DSEN'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('DSEN'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00080][Error] USA documents having DSEN Data must have DSEN at the resource level.'"/>
                  <xsl:text/> 
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M290"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M290"/>
   <xsl:template match="@*|node()" priority="-2" mode="M290">
      <xsl:apply-templates select="*" mode="M290"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00081-->


	<!--RULE AttributeContributesToRollup-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('FISA')))]"
                 priority="1000"
                 mode="M291">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('FISA')))]"
                       id="AttributeContributesToRollup-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('FISA'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('FISA'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00081][Error] USA documents having FISA Data must have FISA at the resource level.'"/>
                  <xsl:text/> 
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M291"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M291"/>
   <xsl:template match="@*|node()" priority="-2" mode="M291">
      <xsl:apply-templates select="*" mode="M291"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00084-->


	<!--RULE AttributeContributesToRollupWithClassification-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE      and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)      and util:containsAnyOfTheTokens(@ism:classification, ( 'U' ))     and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:nonICmarkings, ('DS')))]"
                 priority="1000"
                 mode="M292">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE      and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)      and util:containsAnyOfTheTokens(@ism:classification, ( 'U' ))     and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:nonICmarkings, ('DS')))]"
                       id="AttributeContributesToRollupWithClassification-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:nonICmarkings, ('DS'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:nonICmarkings, ('DS'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			               <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00084][Error] Unclassified USA documents having DS Data must have DS at the resource level.'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M292"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M292"/>
   <xsl:template match="@*|node()" priority="-2" mode="M292">
      <xsl:apply-templates select="*" mode="M292"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00085-->


	<!--RULE AttributeContributesToRollupWithException-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:nonICmarkings, ('ND'))) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:nonICmarkings, ('XD')))]"
                 priority="1000"
                 mode="M293">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:nonICmarkings, ('ND'))) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:nonICmarkings, ('XD')))]"
                       id="AttributeContributesToRollupWithException-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:nonICmarkings, ('XD'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:nonICmarkings, ('XD'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			               <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00085][Error] USA documents having XD Data and not having ND must have XD at the resource level.'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M293"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M293"/>
   <xsl:template match="@*|node()" priority="-2" mode="M293">
      <xsl:apply-templates select="*" mode="M293"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00086-->


	<!--RULE AttributeContributesToRollup-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:nonICmarkings, ('ND')))]"
                 priority="1000"
                 mode="M294">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:nonICmarkings, ('ND')))]"
                       id="AttributeContributesToRollup-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:nonICmarkings, ('ND'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:nonICmarkings, ('ND'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00086][Error] USA documents having ND Data must have ND at the resource level.'"/>
                  <xsl:text/> 
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M294"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M294"/>
   <xsl:template match="@*|node()" priority="-2" mode="M294">
      <xsl:apply-templates select="*" mode="M294"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00087-->


	<!--RULE ISM-ID-00087-R1-->
<xsl:template match="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]"
                 priority="1000"
                 mode="M295">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]"
                       id="ISM-ID-00087-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="if (not($ISM_USGOV_RESOURCE)) then true() else if (index-of($partNonICmarkings_tok, 'SBU-NF') &gt; 0 and not($bannerClassification = 'U')) then (index-of($bannerDisseminationControls_tok, 'NF') &gt; 0) else true()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="if (not($ISM_USGOV_RESOURCE)) then true() else if (index-of($partNonICmarkings_tok, 'SBU-NF') &gt; 0 and not($bannerClassification = 'U')) then (index-of($bannerDisseminationControls_tok, 'NF') &gt; 0) else true()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00087][Error] Classified USA documents having SBU-NF Data must have NF at the resource level. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M295"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M295"/>
   <xsl:template match="@*|node()" priority="-2" mode="M295">
      <xsl:apply-templates select="*" mode="M295"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00088-->


	<!--RULE ISM-ID-00088-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and @ism:releasableTo]"
                 priority="1000"
                 mode="M296">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and @ism:releasableTo]"
                       id="ISM-ID-00088-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $portion in $partTags satisfies ( ($portion/@ism:classification='U' and not($portion/@ism:disseminationControls) ) or $portion/@ism:releasableTo[normalize-space()])"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $portion in $partTags satisfies ( ($portion/@ism:classification='U' and not($portion/@ism:disseminationControls) ) or $portion/@ism:releasableTo[normalize-space()])">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00088][Error] If ISM_USGOV_RESOURCE and @ism:releasableTo is specified on the resource
            element then all classified portions must specify @ism:releasableTo and all Unclass portions must be REL or contain
            no caveats. 
            
            Human Readable: USA documents having any classified portion that is not Releasable or having
            unclassified portions with disseminationControls that are not [REL] cannot be REL at the resource level.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M296"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M296"/>
   <xsl:template match="@*|node()" priority="-2" mode="M296">
      <xsl:apply-templates select="*" mode="M296"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00090-->


	<!--RULE ISM-ID-00090-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and index-of($partDisseminationControls_tok, 'REL') &gt; 0]"
                 priority="1000"
                 mode="M297">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and index-of($partDisseminationControls_tok, 'REL') &gt; 0]"
                       id="ISM-ID-00090-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('EYES')))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('EYES')))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00090][Error] If ISM_USGOV_RESOURCE and any element: 
            1. Meets ISM_CONTRIBUTES
            AND
            2. Has the attribute @ism:disseminationControls containing [REL]
            Then the ISM_RESOURCE_ELEMENT must not have attribute @ism:disseminationControls containing [EYES]. 
            
            Human Readable: USA documents with any portion that is REL must not be EYES at the resource level.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M297"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M297"/>
   <xsl:template match="@*|node()" priority="-2" mode="M297">
      <xsl:apply-templates select="*" mode="M297"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00104-->


	<!--RULE ISM-ID-00104-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and $bannerClassification = 'U' and index-of($partNonICmarkings_tok, 'SBU-NF') &gt; 0 and not(util:containsAnyOfTheTokens(string-join(@ism:nonICmarkings, ' '), ('XD', 'ND'))) and not(util:containsAnyOfTheTokens(string-join(@ism:disseminationControls, ' '), ('NF')))]"
                 priority="1000"
                 mode="M300">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and $bannerClassification = 'U' and index-of($partNonICmarkings_tok, 'SBU-NF') &gt; 0 and not(util:containsAnyOfTheTokens(string-join(@ism:nonICmarkings, ' '), ('XD', 'ND'))) and not(util:containsAnyOfTheTokens(string-join(@ism:disseminationControls, ' '), ('NF')))]"
                       id="ISM-ID-00104-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:nonICmarkings, ('SBU-NF'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:nonICmarkings, ('SBU-NF'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
      [ISM-ID-00104][Error] If the document is an ISM_USGOV_RESOURCE and any
      element in the document is: 
      1. Unclassified and meets ISM_CONTRIBUTES 
      AND 
      2. Has the attribute @ism:nonICmarkings containing [SBU-NF] 
      AND
      3. The ISM_RESOURCE_ELEMENT has attribute @ism:nonICmarkings does not contain [XD] or [ND] 
      AND
      4. The ISM_RESOURCE_ELEMENT has attribute @ism:disseminationControls does not contain [NF]
      Then the ISM_RESOURCE_ELEMENT must have @ism:nonICmarkings containing [SBU-NF]. 
      
      Human Readable: USA Unclassified documents having SBU-NF and not having XD, ND, or explicit Foreign Disclosure and
      Release markings must have SBU-NF at the resource level.
    </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M300"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M300"/>
   <xsl:template match="@*|node()" priority="-2" mode="M300">
      <xsl:apply-templates select="*" mode="M300"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00105-->


	<!--RULE ISM-ID-00105-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and $bannerClassification = 'U' and index-of($partNonICmarkings_tok, 'SBU') &gt; 0 and not(util:containsAnyOfTheTokens(string-join($partNonICmarkings, ' '), ('SBU-NF', 'XD', 'ND')))]"
                 priority="1000"
                 mode="M301">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and $bannerClassification = 'U' and index-of($partNonICmarkings_tok, 'SBU') &gt; 0 and not(util:containsAnyOfTheTokens(string-join($partNonICmarkings, ' '), ('SBU-NF', 'XD', 'ND')))]"
                       id="ISM-ID-00105-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:nonICmarkings, ('SBU'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:nonICmarkings, ('SBU'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
      [ISM-ID-00105][Error] If the document is an ISM_USGOV_RESOURCE and any
      element in the document is: 
      1. Unclassifed and meets ISM_CONTRIBUTES 
      AND 
      2. Has the attribute @ism:nonICmarkings containing [SBU] 
      AND 
      3. No element meeting ISM_CONTRIBUTES in the document has @ism:nonICmarkings containing any of [SBU-NF], 
      [XD], or [ND]
      Then the ISM_RESOURCE_ELEMENT must have @ism:nonICmarkings containing [SBU]. 
      
      Human Readable: USA Unclassified documents having SBU and not having SBU-NF, XD, or ND must have SBU at the resource level.
    </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M301"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M301"/>
   <xsl:template match="@*|node()" priority="-2" mode="M301">
      <xsl:apply-templates select="*" mode="M301"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00145-->


	<!--RULE ISM-ID-00145-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and index-of($partNonICmarkings_tok, 'LES') &gt; 0 and not(index-of($partNonICmarkings_tok, 'LES-NF') &gt; 0)]"
                 priority="1000"
                 mode="M321">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and index-of($partNonICmarkings_tok, 'LES') &gt; 0 and not(index-of($partNonICmarkings_tok, 'LES-NF') &gt; 0)]"
                       id="ISM-ID-00145-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:nonICmarkings, ('LES'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:nonICmarkings, ('LES'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00145][Error] If ISM_USGOV_RESOURCE and any element in the document: 
            1. Meets ISM_CONTRIBUTES
            AND
            2. Has the attribute @ism:nonICmarkings containing [LES]
            AND
            3. No element meeting ISM_CONTRIBUTES in the document has @ism:nonICmarkings containing any of [LES-NF]
            Then the ISM_RESOURCE_ELEMENT must have @ism:nonICmarkings containing [LES].
            
            Human Readable: USA documents having LES and not having LES-NF must have LES at the resource level.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M321"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M321"/>
   <xsl:template match="@*|node()" priority="-2" mode="M321">
      <xsl:apply-templates select="*" mode="M321"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00146-->


	<!--RULE ISM-ID-00146-R1-->
<xsl:template match="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]"
                 priority="1000"
                 mode="M322">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]"
                       id="ISM-ID-00146-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="if(not($ISM_USGOV_RESOURCE)) then true() else if(index-of($partNonICmarkings_tok, 'LES-NF') &gt; 0 and not($bannerClassification='U')) then (index-of($bannerDisseminationControls_tok, 'NF') &gt; 0) else true()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="if(not($ISM_USGOV_RESOURCE)) then true() else if(index-of($partNonICmarkings_tok, 'LES-NF') &gt; 0 and not($bannerClassification='U')) then (index-of($bannerDisseminationControls_tok, 'NF') &gt; 0) else true()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00146][Error] If ISM_USGOV_RESOURCE and there exist at least 2 elements in the document:
            1. Each element: Meets ISM_CONTRIBUTES
            AND
            2. One of the elements: Has the attribute @ism:nonICmarkings containing [LES-NF]
            AND
            3. One of the elements: meets ISM_CONTRIBUTES_CLASSIFIED
            Then the ISM_RESOURCE_ELEMENT must have @ism:disseminationControls containing [NF].
            
            Human Readable: Classified USA documents having LES-NF Data must have NF at the resource level.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M322"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M322"/>
   <xsl:template match="@*|node()" priority="-2" mode="M322">
      <xsl:apply-templates select="*" mode="M322"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00147-->


	<!--RULE ISM-ID-00147-R1-->
<xsl:template match="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]"
                 priority="1000"
                 mode="M323">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]"
                       id="ISM-ID-00147-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="if(not($ISM_USGOV_RESOURCE)) then true() else if(index-of($partNonICmarkings_tok, 'LES-NF') &gt; 0 and not($bannerClassification='U')) then (index-of($bannerNonICmarkings_tok, 'LES') &gt; 0) else true()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="if(not($ISM_USGOV_RESOURCE)) then true() else if(index-of($partNonICmarkings_tok, 'LES-NF') &gt; 0 and not($bannerClassification='U')) then (index-of($bannerNonICmarkings_tok, 'LES') &gt; 0) else true()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00147][Error] If ISM_USGOV_RESOURCE and there exist at least 2 elements in the document:
            1. Each element: Meets ISM_CONTRIBUTES
            AND
            2. One of the elements: Has the attribute @ism:nonICmarkings containing [LES-NF]
            AND
            3. One of the elements: meets ISM_CONTRIBUTES_CLASSIFIED
            Then the ISM_RESOURCE_ELEMENT must have @ism:nonICmarkings containing [LES].
            
            Human Readable: Classified USA documents having LES-NF Data must have LES at the resource level.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M323"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M323"/>
   <xsl:template match="@*|node()" priority="-2" mode="M323">
      <xsl:apply-templates select="*" mode="M323"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00149-->


	<!--RULE ISM-ID-00149-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and $bannerClassification = 'U' and index-of($partNonICmarkings_tok, 'LES-NF') &gt; 0 and not(util:containsAnyOfTheTokens(string-join(@ism:disseminationControls, ' '), ('NF')))]"
                 priority="1000"
                 mode="M325">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and $bannerClassification = 'U' and index-of($partNonICmarkings_tok, 'LES-NF') &gt; 0 and not(util:containsAnyOfTheTokens(string-join(@ism:disseminationControls, ' '), ('NF')))]"
                       id="ISM-ID-00149-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:nonICmarkings, ('LES-NF'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:nonICmarkings, ('LES-NF'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
      [ISM-ID-00149][Error] If the document is an ISM_USGOV_RESOURCE and:
      1. Any element in the document meets ISM_CONTRIBUTES in the document has the attribute @ism:nonICmarkings
      contain [LES-NF] 
      AND 
      2. ISM_RESOURCE_ELEMENT has the attribute @ism:classification [U] 
      AND 
      3. ISM_RESOURCE_ELEMENT does not have the attribute @ism:disseminationControls [NF] 
      THEN the ISM_RESOURCE_ELEMENT must have @ism:nonICmarkings containing [LES-NF]
      
      Human Readable: Unclassified USA documents having LES-NF and not having NF 
      must have LES-NF at the resource level.
    </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M325"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M325"/>
   <xsl:template match="@*|node()" priority="-2" mode="M325">
      <xsl:apply-templates select="*" mode="M325"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00165-->


	<!--RULE AttributeContributesToRollup-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('RS')))]"
                 priority="1000"
                 mode="M333">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('RS')))]"
                       id="AttributeContributesToRollup-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('RS'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('RS'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00165][Error] USA documents having RISK SENSITIVE (RS) data must have RS at the resource level.'"/>
                  <xsl:text/> 
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M333"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M333"/>
   <xsl:template match="@*|node()" priority="-2" mode="M333">
      <xsl:apply-templates select="*" mode="M333"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00176-->


	<!--RULE ISM-ID-00176-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD', 'FRD'))]"
                 priority="1000"
                 mode="M341">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD', 'FRD'))]"
                       id="ISM-ID-00176-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not($ISM_RESOURCE_ELEMENT/@ism:declassDate or $ISM_RESOURCE_ELEMENT/@ism:declassEvent)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not($ISM_RESOURCE_ELEMENT/@ism:declassDate or $ISM_RESOURCE_ELEMENT/@ism:declassEvent)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00176][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:atomicEnergyMarkings has a name token containing [RD] or [FRD], 
            then attributes @ism:declassDate and @ism:declassEvent cannot be specified
            on the resourceElement.
            
            Human Readable: Automatic declassification of documents containing 
            RD or FRD information is prohibited. Attributes declassDate and 
            declassEvent cannot be used in the classification authority block when 
            RD or FRD is present.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M341"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M341"/>
   <xsl:template match="@*|node()" priority="-2" mode="M341">
      <xsl:apply-templates select="*" mode="M341"/>
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

   <!--PATTERN ISM-ID-00298-->


	<!--RULE AttributeContributesToRollupWithException-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:atomicEnergyMarkings, ('RD', 'FRD'))) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:atomicEnergyMarkings, ('TFNI')))]"
                 priority="1000"
                 mode="M433">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:atomicEnergyMarkings, ('RD', 'FRD'))) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:atomicEnergyMarkings, ('TFNI')))]"
                       id="AttributeContributesToRollupWithException-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('TFNI'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('TFNI'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			               <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00298][Error] USA documents having Transclassified Foreign Nuclear Information (TFNI)     and not having Restricted Data (RD) or Formerly Restricted Data (FRD) must have TFNI at the resource level.'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M433"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M433"/>
   <xsl:template match="@*|node()" priority="-2" mode="M433">
      <xsl:apply-templates select="*" mode="M433"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00315-->


	<!--RULE ISM-ID-00315-R1-->
<xsl:template match="*[not(generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)) and util:contributesToRollup(.) and $ISM_USGOV_RESOURCE and not(@ism:classification = 'U') and util:containsAnyTokenMatching(@ism:ownerProducer, ('NATO:?'))]"
                 priority="1000"
                 mode="M439">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[not(generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)) and util:contributesToRollup(.) and $ISM_USGOV_RESOURCE and not(@ism:classification = 'U') and util:containsAnyTokenMatching(@ism:ownerProducer, ('NATO:?'))]"
                       id="ISM-ID-00315-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:declassException, ('NATO', 'NATO-AEA'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:declassException, ('NATO', 'NATO-AEA'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> 
            [ISM-ID-00315][Error] If classified element meets ISM_CONTRIBUTES and
            attribute @ism:ownerProducer contains the token [NATO], then attribute @ism:declassException must be
            specified with a value of [NATO] or [NATO-AEA] on the resourceElement. 
            
            Human Readable: Any document with non-resource classified elements that contributes to the document's banner 
            roll-up and has NATO Information must also specify a NATO declass exemption on the banner. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M439"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M439"/>
   <xsl:template match="@*|node()" priority="-2" mode="M439">
      <xsl:apply-templates select="*" mode="M439"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00318-->


	<!--RULE CheckCommonCountries-R1-->
<xsl:template match="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and @ism:*[local-name() = 'releasableTo']]"
                 priority="1000"
                 mode="M442">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and @ism:*[local-name() = 'releasableTo']]"
                       id="CheckCommonCountries-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count($relToCalculatedBannerTokens) != 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count($relToCalculatedBannerTokens) != 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00318'"/>
                  <xsl:text/>][Error] The banner cannot have @ism:<xsl:text/>
                  <xsl:value-of select="'releasableTo'"/>
                  <xsl:text/> because
      there is no common country in the contributing portions.
    </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="if(count($relToCalculatedBannerTokens) != 0 and @ism:compilationReason[normalize-space(.)])        then util:isSubsetOf($relToActualBannerTokens, $relToCalculatedBannerTokens) else true()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="if(count($relToCalculatedBannerTokens) != 0 and @ism:compilationReason[normalize-space(.)]) then util:isSubsetOf($relToActualBannerTokens, $relToCalculatedBannerTokens) else true()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00318'"/>
                  <xsl:text/>][Error] The banner @ism:<xsl:text/>
                  <xsl:value-of select="'releasableTo'"/>
                  <xsl:text/> must be a subset of the 
      common countries for contributing portions because @ism:compilationReason is specified. Common countries: [<xsl:text/>
                  <xsl:value-of select="util:join($relToCalculatedBannerTokens)"/>
                  <xsl:text/>].
    </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="if(count($relToCalculatedBannerTokens) != 0 and not(@ism:compilationReason[normalize-space(.)]))        then util:join(util:sort($relToCalculatedBannerTokens)) = util:join(util:sort($relToActualBannerTokens)) else true()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="if(count($relToCalculatedBannerTokens) != 0 and not(@ism:compilationReason[normalize-space(.)])) then util:join(util:sort($relToCalculatedBannerTokens)) = util:join(util:sort($relToActualBannerTokens)) else true()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00318'"/>
                  <xsl:text/>][Error] The banner @ism:<xsl:text/>
                  <xsl:value-of select="'releasableTo'"/>
                  <xsl:text/> must match the set of the common countries for 
      contributing portions because @ism:compilationReason is not specified. Common countries: [<xsl:text/>
                  <xsl:value-of select="util:join($relToCalculatedBannerTokens)"/>
                  <xsl:text/>].
    </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M442"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M442"/>
   <xsl:template match="@*|node()" priority="-2" mode="M442">
      <xsl:apply-templates select="*" mode="M442"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00320-->


	<!--RULE CheckCommonCountries-R1-->
<xsl:template match="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and @ism:*[local-name() = 'displayOnlyTo']]"
                 priority="1000"
                 mode="M444">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and @ism:*[local-name() = 'displayOnlyTo']]"
                       id="CheckCommonCountries-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count($displayToCalculatedBannerTokens) != 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count($displayToCalculatedBannerTokens) != 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00320'"/>
                  <xsl:text/>][Error] The banner cannot have @ism:<xsl:text/>
                  <xsl:value-of select="'displayOnlyTo'"/>
                  <xsl:text/> because
      there is no common country in the contributing portions.
    </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="if(count($displayToCalculatedBannerTokens) != 0 and @ism:compilationReason[normalize-space(.)])        then util:isSubsetOf($displayToActualBannerTokens, $displayToCalculatedBannerTokens) else true()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="if(count($displayToCalculatedBannerTokens) != 0 and @ism:compilationReason[normalize-space(.)]) then util:isSubsetOf($displayToActualBannerTokens, $displayToCalculatedBannerTokens) else true()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00320'"/>
                  <xsl:text/>][Error] The banner @ism:<xsl:text/>
                  <xsl:value-of select="'displayOnlyTo'"/>
                  <xsl:text/> must be a subset of the 
      common countries for contributing portions because @ism:compilationReason is specified. Common countries: [<xsl:text/>
                  <xsl:value-of select="util:join($displayToCalculatedBannerTokens)"/>
                  <xsl:text/>].
    </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="if(count($displayToCalculatedBannerTokens) != 0 and not(@ism:compilationReason[normalize-space(.)]))        then util:join(util:sort($displayToCalculatedBannerTokens)) = util:join(util:sort($displayToActualBannerTokens)) else true()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="if(count($displayToCalculatedBannerTokens) != 0 and not(@ism:compilationReason[normalize-space(.)])) then util:join(util:sort($displayToCalculatedBannerTokens)) = util:join(util:sort($displayToActualBannerTokens)) else true()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00320'"/>
                  <xsl:text/>][Error] The banner @ism:<xsl:text/>
                  <xsl:value-of select="'displayOnlyTo'"/>
                  <xsl:text/> must match the set of the common countries for 
      contributing portions because @ism:compilationReason is not specified. Common countries: [<xsl:text/>
                  <xsl:value-of select="util:join($displayToCalculatedBannerTokens)"/>
                  <xsl:text/>].
    </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M444"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M444"/>
   <xsl:template match="@*|node()" priority="-2" mode="M444">
      <xsl:apply-templates select="*" mode="M444"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00343-->


	<!--RULE ISM-ID-00343-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and count($partSCIcontrols_tok)&gt;0]"
                 priority="1000"
                 mode="M456">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and count($partSCIcontrols_tok)&gt;0]"
                       id="ISM-ID-00343-R1"/>
      <xsl:variable name="missingSCI"
                    select="for $token in distinct-values($partSCIcontrols) return  if (index-of(tokenize(@ism:SCIcontrols,' '), $token) &gt; 0 ) then null else $token"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count($missingSCI)=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="count($missingSCI)=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00343][Error] All SCI controls specified in the document that contribute to rollup must
            be rolled up to the resource level. The following tokens were found to be missing from the resource
            element: <xsl:text/>
                  <xsl:value-of select="string-join($missingSCI, ', ')"/>
                  <xsl:text/>.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M456"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M456"/>
   <xsl:template match="@*|node()" priority="-2" mode="M456">
      <xsl:apply-templates select="*" mode="M456"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00347-->


	<!--RULE ISM-ID-00347-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and count($partSARIdentifier_tok)&gt;0]"
                 priority="1000"
                 mode="M460">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and count($partSARIdentifier_tok)&gt;0]"
                       id="ISM-ID-00347-R1"/>
      <xsl:variable name="missingSAR"
                    select="for $token in distinct-values($partSARIdentifier) return if (index-of(tokenize(@ism:SARIdentifier,' '), $token) &gt; 0 ) then null else $token"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count($missingSAR)=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="count($missingSAR)=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00347][Error] All SAR Identifiers specified in the document that contribute to rollup must
            be rolled up to the resource level. The following tokens were found to be missing from the resource
            element: <xsl:text/>
                  <xsl:value-of select="string-join($missingSAR, ', ')"/>
                  <xsl:text/>.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M460"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M460"/>
   <xsl:template match="@*|node()" priority="-2" mode="M460">
      <xsl:apply-templates select="*" mode="M460"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00373-->


	<!--RULE AttributeContributesToRollup-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:nonICmarkings, ('SSI')))]"
                 priority="1000"
                 mode="M482">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:nonICmarkings, ('SSI')))]"
                       id="AttributeContributesToRollup-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:nonICmarkings, ('SSI'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:nonICmarkings, ('SSI'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00373][Error] USA documents having SSI Data must have SSI at the resource level.'"/>
                  <xsl:text/> 
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M482"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M482"/>
   <xsl:template match="@*|node()" priority="-2" mode="M482">
      <xsl:apply-templates select="*" mode="M482"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00389-->


	<!--RULE AttributeContributesToRollup-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('RAWFISA')))]"
                 priority="1000"
                 mode="M491">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('RAWFISA')))]"
                       id="AttributeContributesToRollup-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('RAWFISA'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('RAWFISA'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00389][Error] USA documents having RAWFISA Data must have RAWFISA at the resource level.'"/>
                  <xsl:text/> 
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M491"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M491"/>
   <xsl:template match="@*|node()" priority="-2" mode="M491">
      <xsl:apply-templates select="*" mode="M491"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00461-->


	<!--RULE ISM-ID-00461-R1-->
<xsl:template match="*[$ISM_USDOD_RESOURCE and not($ISM_DOD_DISTRO_EXEMPT) and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (count($partNoticeType_tok[.='ITAR-EAR'])&gt;0)]"
                 priority="1000"
                 mode="M505">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USDOD_RESOURCE and not($ISM_DOD_DISTRO_EXEMPT) and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (count($partNoticeType_tok[.='ITAR-EAR'])&gt;0)]"
                       id="ISM-ID-00461-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of(tokenize(@ism:noticeType,' '), 'ITAR-EAR') &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of(tokenize(@ism:noticeType,' '), 'ITAR-EAR') &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
      [ISM-ID-00461][Error] If ISM_USDOD_RESOURCE and 
      1. not ISM_DOD_DISTRO_EXEMPT
      AND 
      2. Attribute @ism:noticeType of any portion that is not @ism:excludeFromRollup="true" contains [ITAR-EAR],
      then there must be @ism:noticeType=[ITAR-EAR] on the resource element. 
      
      Human Readable: All US DOD documents that do not claim exemption from DoD5230.24 and that have an [ITAR-EAR] notice
      on any portion must have an [ITAR-EAR] notice on the resource element. 
    </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M505"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M505"/>
   <xsl:template match="@*|node()" priority="-2" mode="M505">
      <xsl:apply-templates select="*" mode="M505"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00502-->


	<!--RULE ISM-ID-00502-R1-->
<xsl:template match="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and count($partCuiBasic_tok) &gt; 0]"
                 priority="1000"
                 mode="M542">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and count($partCuiBasic_tok) &gt; 0]"
                       id="ISM-ID-00502-R1"/>
      <xsl:variable name="missingCuiBasic"
                    select="for $token in distinct-values($partCuiBasic) return if (index-of(tokenize(@ism:cuiBasic, ' '), $token) &gt; 0) then null else  $token"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count($missingCuiBasic) = 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count($missingCuiBasic) = 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> 
            [ISM-ID-00502][Error] All CUI Basic category markings specified in the document that contribute to rollup must be rolled up
            to the resource level. The following tokens were found to be missing from the resource
            element: <xsl:text/>
                  <xsl:value-of select="string-join($missingCuiBasic, ', ')"/>
                  <xsl:text/>.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M542"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M542"/>
   <xsl:template match="@*|node()" priority="-2" mode="M542">
      <xsl:apply-templates select="*" mode="M542"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00503-->


	<!--RULE ISM-ID-00503-R1-->
<xsl:template match="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and count($partCuiSpecified_tok) &gt; 0]"
                 priority="1000"
                 mode="M543">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and count($partCuiSpecified_tok) &gt; 0]"
                       id="ISM-ID-00503-R1"/>
      <xsl:variable name="missingCuiSpecified"
                    select="for $token in distinct-values($partCuiSpecified) return if (index-of(tokenize(@ism:cuiSpecified, ' '), $token) &gt; 0) then null else $token"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count($missingCuiSpecified) = 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count($missingCuiSpecified) = 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> 
            [ISM-ID-00503][Error] All CUI Specified category markings in document portions that contribute to rollup must be rolled up
            to the resource level. The following tokens were found to be missing from the resource
            element: <xsl:text/>
                  <xsl:value-of select="string-join($missingCuiSpecified, ', ')"/>
                  <xsl:text/>.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M543"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M543"/>
   <xsl:template match="@*|node()" priority="-2" mode="M543">
      <xsl:apply-templates select="*" mode="M543"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00521-->


	<!--RULE ISM-ID-00521-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)         and index-of($partDisseminationControls_tok, 'REL') &gt; 0]"
                 priority="1000"
                 mode="M556">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)         and index-of($partDisseminationControls_tok, 'REL') &gt; 0]"
                       id="ISM-ID-00521-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('REL','DISPLAYONLY','NF'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('REL','DISPLAYONLY','NF'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00521][Error] If ISM_USGOV_RESOURCE and any element: 
            1. Meets ISM_CONTRIBUTES
            AND
            2. Has the attribute @ism:disseminationControls containing [REL]
            Then the ISM_RESOURCE_ELEMENT must have attribute @ism:disseminationControls containing either [REL], [DISPLAYONLY] or [NF]. 
            
            Human Readable: USA documents with any portion that is REL must be one of REL, DISPLAYONLY or NF at the resource level.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M556"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M556"/>
   <xsl:template match="@*|node()" priority="-2" mode="M556">
      <xsl:apply-templates select="*" mode="M556"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00528-->


	<!--RULE ISM-ID-00528-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)           and index-of($dcTagsFound,'EXEMPT_FROM_ICD501_DISCOVERY') &gt; 0]"
                 priority="1000"
                 mode="M562">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)           and index-of($dcTagsFound,'EXEMPT_FROM_ICD501_DISCOVERY') &gt; 0]"
                       id="ISM-ID-00528-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('EXEMPT_FROM_ICD501_DISCOVERY'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('EXEMPT_FROM_ICD501_DISCOVERY'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [ISM-ID-00528][Error] If the token
            [EXEMPT_FROM_ICD501_DISCOVERY] is found in any @ism:disseminationControls in portions
            that contribute to rollup, then @disseminationControls=[EXEMPT_FROM_ICD501_DISCOVERY]
            must be rolled up to the resource level. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M562"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M562"/>
   <xsl:template match="@*|node()" priority="-2" mode="M562">
      <xsl:apply-templates select="*" mode="M562"/>
   </xsl:template>
</xsl:stylesheet>
<!--UNCLASSIFIED-->
