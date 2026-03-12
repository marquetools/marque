<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00237">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00237][Error] If ISM_USDOD_RESOURCE, any element which specifies
        attribute @ism:noticeType containing one of the tokens [DoD-Dist-B], 
       	[DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F]
       	must also specify attribute @ism:noticeDate.     	
        
        Human Readable: DoD distribution statements B, C, D, E, and F all require a date.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    	If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:noticeType specified with a value containing the token
        [DoD-Dist-B], [DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F], 
        this rule ensures that attribute @ism:noticeDate is specified.
    </sch:p>
    <sch:rule id="ISM-ID-00237-R1" context="*[$ISM_USDOD_RESOURCE and util:containsAnyOfTheTokens(@ism:noticeType, ('DoD-Dist-B', 'DoD-Dist-C', 'DoD-Dist-D', 'DoD-Dist-E', 'DoD-Dist-F'))]">
        <sch:assert test="@ism:noticeDate" flag="error" role="error"> 
            [ISM-ID-00237][Error] If ISM_USDOD_RESOURCE, any element which specifies
            attribute @ism:noticeType containing one of the tokens [DoD-Dist-B], 
            [DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F]
            must also specify attribute @ism:noticeDate.     	
            
            Human Readable: DoD distribution statements B, C, D, E, and F all require a date.
        </sch:assert>
    </sch:rule>
</sch:pattern>