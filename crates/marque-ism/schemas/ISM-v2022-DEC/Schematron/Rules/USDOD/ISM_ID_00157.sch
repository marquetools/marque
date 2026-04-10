<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00157">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00157][Error] If ISM_USDOD_RESOURCE and: 
        1. The attribute notice contains one of the [DoD-Dist-B], [DoD-Dist-C], [DoD-Dist-D], or [DoD-Dist-E] 
          AND
        2. The attribute @ism:noticeReason is not specified. 
        
        Human Readable: DoD distribution statements B, C, D , or E all require a reason. 
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USDOD_RESOURCE, for each element which
        specifies attribute ism:noticeType with a value containing the token [DoD-Dist-B],
        [DoD-Dist-C], [DoD-Dist-D], or [DoD-Dist-E], this rule ensures that attribute
        @ism:noticeReason is specified. 
    </sch:p>
    <sch:rule id="ISM-ID-00157-R1" context="*[$ISM_USDOD_RESOURCE and util:containsAnyOfTheTokens(@ism:noticeType, ('DoD-Dist-B', 'DoD-Dist-C', 'DoD-Dist-D', 'DoD-Dist-E'))]">
        <sch:assert test="@ism:noticeReason" flag="error" role="error">
            [ISM-ID-00157][Error] If ISM_USDOD_RESOURCE and: 
            1. The attribute notice contains one of the [DoD-Dist-B], [DoD-Dist-C], [DoD-Dist-D], or [DoD-Dist-E] 
            AND
            2. The attribute @ism:noticeReason is not specified. 
            
            Human Readable: DoD distribution statements B, C, D , or E all require a reason. 
        </sch:assert>
    </sch:rule>
</sch:pattern>