<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00037">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00037][Error] When ISM_USGOV_RESOURCE and @ism:nonICmarkings
        contains [SBU] or [SBU-NF] then @ism:classification must equal [U]. 
        
        Human Readable: SBU and SBU-NF data must be marked UNCLASSIFIED on the banner in USA documents.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For a resource element (@ism:resourceElement="true"), if @ism:compliesWith contains ‘USGov’ 
        and @ism:nonICmarkings contains [SBU] or [SBU-NF] then @ism:classification must equal [U].
    </sch:p>
    <sch:rule id="ISM-ID-00037-R1" context="*[$ISM_USGOV_RESOURCE and @ism:resourceElement=true() and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('SBU', 'SBU-NF'))]">
        <sch:assert test="@ism:classification='U'" flag="error" role="error">
            [ISM-ID-00037][Error] When ISM_USGOV_RESOURCE and @ism:nonICmarkings
            contains [SBU] or [SBU-NF] then @ism:classification must equal [U]. 
            
            Human Readable: SBU and SBU-NF data must be marked UNCLASSIFIED on the banner in USA documents.</sch:assert>
    </sch:rule>
</sch:pattern>