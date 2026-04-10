<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?><?schematron-phases phaseids="ROLLDOWN VALUECHECK"?><!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       --><sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00504">
    <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="ruleText">
        [ISM-ID-00504][Error] If ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, and
        there exists a token in @ism:cuiBasic on the ISM_RESOURCE_ELEMENT and no compilation reason,
        then the token must also be specified in the @ism:cuiBasic attribute on at least one
        portion. 
        
        Human Readable: All CUI Basic category markings specified at the resource level
        must be found in a contributing portion of the document unless there is a compilation reason
        of the exception. 
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="codeDesc">If ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, and attribute @ism:cuiBasic
        of ISM_RESOURCE_ELEMENT exists and attribute @ism:compilationReason does not have a value,
        then this rule ensures that at least one element meeting ISM_CONTRIBUTES specifies attribute
        @ism:cuiBasic with each value specified on the ISM_RESOURCE_ELEMENT. 
    </sch:p>
    <sch:rule id="ISM-ID-00504-R1"
             context="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and @ism:cuiBasic and string-length(normalize-space(@ism:compilationReason)) = 0]">
        <sch:let name="missingCuiBasic"
               value="for $token in tokenize(@ism:cuiBasic, ' ') return if (index-of(distinct-values($partCuiBasic), $token) &gt; 0) then null else $token"/>
        <sch:assert test="count($missingCuiBasic) = 0" flag="error" role="error"> 
            [ISM-ID-00504][Error] All CUI Basic category markings specified at the resource level 
            must be found in a contributing portion of the document unless there is a compilation reason of the exception. 
            The following tokens were found to be missing from the portions: <sch:value-of select="string-join($missingCuiBasic, ', ')"/>. 
        </sch:assert>
    </sch:rule>
</sch:pattern>
