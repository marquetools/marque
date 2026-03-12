<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00479">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00479][Error] If @ism:compliesWith contains "USA-CUI" then there MUST be some other token in ism:compliesWith.  
        
        Human Readable: If a document contains USA-CUI but is not USA-CUI-ONLY, 
        then it must comply with some other authority such as USDOD.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    	If the document has @ism:compliesWith that contains "USA-CUI", this rule ensures that there is some other token 
    	in @ism:compliesWith.  A document cannot have ism:compliesWith='USA-CUI'. It must have something like 
    	ism:compliesWith='CUI USGov USDOD'.
    </sch:p>
    <sch:rule id="ISM-ID-00479-R1" context="//*[contains(@ism:compliesWith,'USA-CUI')]">
        <sch:assert test="not(./@ism:compliesWith='USA-CUI')" flag="error" role="error">
            [ISM-ID-00479][Error] If @ism:compliesWith contains "USA-CUI" then there MUST some other token in ism:compliesWith.  
            
            Human Readable: If a document contains USA-CUI but is not USA-CUI-ONLY, 
            then it must comply with some other authority such as USDOD.
        </sch:assert>
    </sch:rule>
</sch:pattern>