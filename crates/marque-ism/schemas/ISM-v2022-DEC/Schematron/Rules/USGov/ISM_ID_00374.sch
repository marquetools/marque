<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLDOWN VALUECHECK"?> 
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00374">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText"> 
        [ISM-ID-00374][Error] If ISM_USGOV_RESOURCE and @ism:nonICmarkings contains 'SSI' on the ISM_RESOURCE_ELEMENT
        with no compilation reason then the token 'SSI' must exist in an @ism:nonICmarkings attribute
        on at least one portion. 
         
        Human Readable: If @ism:nonICmarkings contains 'SSI' at the resource level, it must be found in a contributing
        portion of the document unless there is a compilation reason of the exception.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If ISM_USGOV_RESOURCE and attribute @ism:nonICmarkings contains 'SSI' 
        on the ISM_RESOURCE_ELEMENT and attribute @ism:compilationReason does not have a
        value, then this rule ensures that at least one element meeting ISM_CONTRIBUTES has attribute
        @ism:nonICmarkings containing 'SSI'.
    </sch:p>
    <sch:rule id="ISM-ID-00374-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('SSI')) and string-length(normalize-space(@ism:compilationReason)) = 0]"> 
        <sch:assert test="some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:nonICmarkings, ('SSI'))" flag="error" role="error">
            [ISM-ID-00374][Error] If ISM_USGOV_RESOURCE and @ism:nonICmarkings contains 'SSI' on the ISM_RESOURCE_ELEMENT
            with no compilation reason then the token 'SSI' must exist in an @ism:nonICmarkings attribute
            on at least one portion. 
            
            Human Readable: If @ism:nonICmarkings contains 'SSI' at the resource level, it must be found in a contributing
            portion of the document unless there is a compilation reason of the exception.
        </sch:assert> 
    </sch:rule>
</sch:pattern>